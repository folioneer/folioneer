//! The program an agent client starts with `--mcp` (ADR-023, AGT-010): it holds no data.
//! It speaks the Model Context Protocol on its standard input and output, and passes each
//! tool call to the running application over the local channel.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use super::definitions;
use super::mcp::{Conversation, ToolHost, ToolOutcome};
use super::wire::{read_frame, write_frame, Answer, Request, MAX_FRAME_BYTES};

/// AGT-011 — what an agent reads when no application answers.
pub const NOT_OPEN: &str = "Folioneer is not open, or agents are not allowed to connect: open Folioneer and switch on \"Allow agents to connect\" in its settings.";
/// AGT-032 — what an agent reads when the owner refuses the connection.
pub const NOT_ALLOWED: &str = "The connection was refused in Folioneer.";
/// AGT-034 — what an agent reads when the application ends the connection.
pub const CLOSED: &str = "Folioneer closed the connection.";

/// Both directions of the channel to the running application.
pub trait Channel: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Channel for T {}

/// Opens the channel to the running application; `None` when none answers.
pub type Connector =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = Option<Box<dyn Channel>>> + Send>> + Send>;

/// The running application, reached over the channel: connected at the first tool call,
/// then kept until either side closes it.
pub struct Application {
    connector: Connector,
    channel: Option<BufReader<Box<dyn Channel>>>,
    /// Whether the owner allowed the connection the channel holds (AGT-085).
    granted: Arc<AtomicBool>,
}

impl Application {
    /// The application `connector` reaches.
    pub fn new(connector: Connector) -> Self {
        Self {
            connector,
            channel: None,
            granted: Arc::new(AtomicBool::new(false)),
        }
    }

    /// AGT-030 — opens the channel and asks to connect as `client`; the answer comes when
    /// the owner gives it in the window.
    async fn connect(&mut self, client: &str) -> Result<(), ToolOutcome> {
        let Some(channel) = (self.connector)().await else {
            return Err(ToolOutcome::refusal(NOT_OPEN));
        };
        let mut channel = BufReader::new(channel);
        let asked = Request::Connect {
            client: client.to_string(),
        };
        match exchange(&mut channel, &asked).await {
            Some(Answer::Granted) => {
                self.channel = Some(channel);
                self.granted.store(true, Ordering::SeqCst);
                Ok(())
            }
            Some(Answer::Refused { .. }) => Err(ToolOutcome::refusal(NOT_ALLOWED)),
            Some(Answer::Result { .. }) | None => Err(ToolOutcome::refusal(NOT_OPEN)),
        }
    }
}

#[async_trait::async_trait]
impl ToolHost for Application {
    async fn call(&mut self, client: &str, tool: &str, arguments: Value) -> ToolOutcome {
        if self.channel.is_none() {
            if let Err(refusal) = self.connect(client).await {
                return refusal;
            }
        }
        let Some(channel) = self.channel.as_mut() else {
            return ToolOutcome::refusal(NOT_OPEN);
        };
        let call = Request::Call {
            tool: tool.to_string(),
            arguments,
        };
        match exchange(channel, &call).await {
            Some(Answer::Result { content }) => ToolOutcome {
                text: content.to_string(),
                refused: false,
            },
            Some(Answer::Refused { message, .. }) => ToolOutcome::refusal(message),
            // AGT-034 — the grant ended with the channel: the next call asks again.
            Some(Answer::Granted) | None => {
                self.channel = None;
                self.granted.store(false, Ordering::SeqCst);
                ToolOutcome::refusal(CLOSED)
            }
        }
    }
}

/// Sends `request` and reads its answer; `None` when the channel fails or closes.
async fn exchange<C>(channel: &mut C, request: &Request) -> Option<Answer>
where
    C: AsyncBufRead + AsyncWrite + Unpin,
{
    write_frame(channel, request).await.ok()?;
    read_frame(channel).await.ok().flatten()
}

/// AGT-085 — how many lines a client may send while a call waits before the bridge stops
/// reading it: what it sends next waits in the input until the call is answered.
const WAITING_LINES: usize = 64;

/// AGT-010 — serves one agent client: reads its messages from `input` until it ends,
/// answers on `output`, and passes tool calls to the application `connector` reaches. A
/// line that is not JSON is skipped. While a message is answered the client is still read:
/// what it says meanwhile waits its turn, a cancel of the tool call waiting for the owner's
/// answer to its connection abandons that call (AGT-085), and a client that leaves ends
/// the bridge.
pub async fn serve<I, O>(input: I, mut output: O, connector: Connector) -> std::io::Result<()>
where
    I: AsyncRead + Unpin,
    O: AsyncWrite + Unpin,
{
    let mut input = BufReader::new(input);
    let mut conversation = Conversation::new(definitions::definitions());
    let mut application = Application::new(connector);
    let granted = Arc::clone(&application.granted);
    let mut later: VecDeque<String> = VecDeque::new();
    // A line read in part while a call waits; the rest of it is read next.
    let mut partial: Vec<u8> = Vec::new();
    loop {
        let line = match later.pop_front() {
            Some(line) => line,
            None => match rest_of_line(&mut input, &mut partial).await? {
                Some(line) => line,
                None => break,
            },
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = message.get("id").cloned();
        let a_call = message.get("method").and_then(Value::as_str) == Some("tools/call");
        let answer = {
            let answering = conversation.answer(&message, &mut application);
            tokio::pin!(answering);
            loop {
                tokio::select! {
                    // An answer that is there is given before anything else is read.
                    biased;
                    answer = &mut answering => break answer,
                    more = input.fill_buf(), if later.len() < WAITING_LINES => {
                        let available = more?;
                        if available.is_empty() {
                            return Ok(());
                        }
                        let (taken, ended) = take_line_part(available, &mut partial)?;
                        input.consume(taken);
                        if !ended {
                            continue;
                        }
                        let line = whole_line(std::mem::take(&mut partial))?;
                        // AGT-085 — a tool call given up on while the bridge holds no grant
                        // is abandoned: the request is withdrawn with the channel, and the
                        // call takes no answer. Once granted, a cancel changes nothing.
                        if a_call && !granted.load(Ordering::SeqCst) && cancels(&line, id.as_ref()) {
                            break None;
                        }
                        later.push_back(line);
                    }
                }
            }
        };
        if let Some(answer) = answer {
            let mut text = answer.to_string();
            text.push('\n');
            output.write_all(text.as_bytes()).await?;
            output.flush().await?;
        }
    }
    Ok(())
}

/// Takes from `available` up to the end of a line into `partial`: how many bytes it took,
/// and whether the line ended. A line longer than `MAX_FRAME_BYTES` is refused.
fn take_line_part(available: &[u8], partial: &mut Vec<u8>) -> std::io::Result<(usize, bool)> {
    let end = available.iter().position(|byte| *byte == b'\n');
    let taken = end.map_or(available.len(), |position| position + 1);
    partial.extend_from_slice(available.get(..taken).unwrap_or(available));
    if partial.len() > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the line is too long",
        ));
    }
    Ok((taken, end.is_some()))
}

/// The text of a line read whole, without its end.
fn whole_line(bytes: Vec<u8>) -> std::io::Result<String> {
    String::from_utf8(bytes)
        .map(|text| text.trim_end_matches(['\n', '\r']).to_string())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

/// The next line, its first part already in `partial`: `Ok(None)` at the end of the input.
async fn rest_of_line<R>(input: &mut R, partial: &mut Vec<u8>) -> std::io::Result<Option<String>>
where
    R: AsyncBufRead + Unpin,
{
    loop {
        let available = input.fill_buf().await?;
        if available.is_empty() {
            break;
        }
        let (taken, ended) = take_line_part(available, partial)?;
        input.consume(taken);
        if ended {
            break;
        }
    }
    if partial.is_empty() {
        return Ok(None);
    }
    whole_line(std::mem::take(partial)).map(Some)
}

/// Whether `line` is the client's cancel of the request `id` (`notifications/cancelled`).
fn cancels(line: &str, id: Option<&Value>) -> bool {
    let Some(id) = id else {
        return false;
    };
    serde_json::from_str::<Value>(line).is_ok_and(|message| {
        message.get("method").and_then(Value::as_str) == Some("notifications/cancelled")
            && message
                .get("params")
                .and_then(|params| params.get("requestId"))
                == Some(id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use tokio::io::{duplex, AsyncBufReadExt, DuplexStream};

    fn nobody() -> Connector {
        Box::new(|| Box::pin(async { None }))
    }

    /// A connector handing out the given ends of channels, one per connection.
    fn reaching(ends: Vec<DuplexStream>) -> Connector {
        let ends = Arc::new(Mutex::new(ends));
        Box::new(move || {
            let ends = Arc::clone(&ends);
            Box::pin(async move {
                let end = ends.lock().expect("ends").pop()?;
                Some(Box::new(end) as Box<dyn Channel>)
            })
        })
    }

    /// What the bridge answers a client that sends `client` and keeps its input open until
    /// each request in it is answered, as a client does (AGT-085).
    async fn said(client: &str, connector: Connector) -> Vec<Value> {
        let requests = client
            .lines()
            .filter(|line| {
                serde_json::from_str::<Value>(line).is_ok_and(|message| message.get("id").is_some())
            })
            .count();
        let (mut writing, input) = duplex(64 * 1024);
        let (output, reading) = duplex(64 * 1024);
        let served = tokio::spawn(serve(input, output, connector));
        writing.write_all(client.as_bytes()).await.expect("said");
        let mut reading = tokio::io::BufReader::new(reading);
        let mut answers = Vec::with_capacity(requests);
        for _ in 0..requests {
            let mut line = String::new();
            reading.read_line(&mut line).await.expect("read");
            answers.push(serde_json::from_str(&line).expect("json"));
        }
        drop(writing);
        served.await.expect("served").expect("ended");
        answers
    }

    const CALL: &str = r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"list_accounts","arguments":{}}}"#;

    // ADR-023 / AGT-011 — with no application answering, the bridge still introduces itself
    // and lists its tools, and a tool call is answered that the application is not open.
    #[tokio::test]
    async fn adr_023_the_bridge_answers_that_the_application_is_not_open() {
        let client = format!(
            "{}\nnot json\n{}\n{CALL}\n",
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"claude-code"}}}"#,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        );

        let answers = said(&client, nobody()).await;

        assert_eq!(answers.len(), 3);
        assert_eq!(answers[0]["result"]["serverInfo"]["name"], "folioneer");
        let listed: Vec<&str> = answers[1]["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .map(|tool| tool["name"].as_str().expect("name"))
            .collect();
        assert_eq!(
            listed,
            vec![
                "portfolio_summary",
                "list_accounts",
                "list_assets",
                "list_holdings",
                "list_transactions",
                "get_price",
                "record_opening_balance",
                "record_purchase",
                "record_sale",
                "record_deposit",
                "record_withdrawal",
                "record_dividend",
                "record_split",
                "record_free_shares",
                "record_write_off",
                "record_transfer",
                "record_interest",
                "record_management_fee",
                "record_price",
                "add_asset",
                "add_account",
                "correct_recording",
                "cancel_recording",
                "preview_recording",
                "request_removal",
                "request_change",
                "request_transfer_replacement",
            ]
        );
        assert_eq!(
            answers[2]["result"],
            json!({ "content": [{ "type": "text", "text": NOT_OPEN }], "isError": true })
        );
    }

    /// The application's end of a channel, played by the test: reads the requests it is
    /// sent and answers each with the next of `answers`, then closes.
    fn application_answering(
        answers: Vec<Answer>,
    ) -> (DuplexStream, tokio::task::JoinHandle<Vec<Request>>) {
        let (bridge_end, application_end) = duplex(64 * 1024);
        let played = tokio::spawn(async move {
            let (reading, mut writing) = tokio::io::split(application_end);
            let mut reading = BufReader::new(reading);
            let mut seen = Vec::new();
            for answer in answers {
                let mut line = String::new();
                if reading.read_line(&mut line).await.unwrap_or(0) == 0 {
                    break;
                }
                seen.push(serde_json::from_str(&line).expect("request"));
                write_frame(&mut writing, &answer).await.expect("answered");
            }
            seen
        });
        (bridge_end, played)
    }

    // AGT-030 / AGT-040 — the first tool call asks to connect under the client's name; once
    // the owner allows, that call and the next go through on the same connection.
    #[tokio::test]
    async fn agt_030_the_first_call_connects_and_the_grant_covers_the_next() {
        let (end, application) = application_answering(vec![
            Answer::Granted,
            Answer::Result {
                content: json!([{ "name": "PEA" }]),
            },
            Answer::Refused {
                code: "AccountNotFound".to_string(),
                message: "no account named \"X\"".to_string(),
            },
        ]);
        let client = format!(
            "{}\n{CALL}\n{CALL}\n",
            r#"{"id":1,"method":"initialize","params":{"clientInfo":{"name":"claude-code"}}}"#
        );

        let answers = said(&client, reaching(vec![end])).await;

        assert_eq!(
            answers[1]["result"],
            json!({ "content": [{ "type": "text", "text": "[{\"name\":\"PEA\"}]" }], "isError": false })
        );
        assert_eq!(answers[2]["result"]["isError"], true);
        assert_eq!(
            answers[2]["result"]["content"][0]["text"],
            "no account named \"X\""
        );
        let seen = application.await.expect("application");
        assert_eq!(
            seen.first(),
            Some(&Request::Connect {
                client: "claude-code".to_string()
            })
        );
        assert_eq!(seen.len(), 3);
    }

    // AGT-032 / AGT-034 — a refusal gives the agent nothing and the next call asks again;
    // a connection the application closes ends the grant the same way.
    #[tokio::test]
    async fn agt_032_a_refused_or_closed_connection_gives_nothing_and_is_asked_again() {
        let (refusing, refused) = application_answering(vec![Answer::Refused {
            code: "ConnectionRefused".to_string(),
            message: "refused".to_string(),
        }]);
        let (closing, closed) = application_answering(vec![Answer::Granted]);
        let (allowing, allowed) =
            application_answering(vec![Answer::Granted, Answer::Result { content: json!([]) }]);
        let client = format!("{CALL}\n{CALL}\n{CALL}\n");

        // Connections are handed out from the end of the list.
        let answers = said(&client, reaching(vec![allowing, closing, refusing])).await;

        let text = |index: usize| answers[index]["result"]["content"][0]["text"].clone();
        assert_eq!(text(0), NOT_ALLOWED);
        assert_eq!(text(1), CLOSED);
        assert_eq!(text(2), "[]");
        assert_eq!(answers[2]["result"]["isError"], false);
        assert_eq!(refused.await.expect("played").len(), 1);
        assert_eq!(closed.await.expect("played").len(), 1);
        assert_eq!(allowed.await.expect("played").len(), 2);
    }
}
