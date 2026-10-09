//! What the bridge and the running application say to each other over the local channel
//! (AGT-020): one JSON object per line, a request then its answer.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

/// The longest line either side reads: a request or an answer beyond it ends the exchange.
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// What the bridge asks of the application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// An agent client asks to connect; the owner answers in the window (AGT-030).
    Connect {
        /// The name the agent client gave for itself.
        client: String,
    },
    /// A connected agent calls a tool (AGT-040).
    Call {
        /// The tool's name.
        tool: String,
        /// The tool's arguments, as the agent sent them.
        arguments: serde_json::Value,
    },
}

/// What the application answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    /// The owner allowed the connection.
    Granted,
    /// The tool ran; `content` is its result.
    Result {
        /// The tool's result.
        content: serde_json::Value,
    },
    /// The connection or the call was refused.
    Refused {
        /// A stable code for the reason.
        code: String,
        /// The reason, for the agent to read.
        message: String,
    },
}

/// The name an agent is shown under when it gives none.
pub const UNNAMED_AGENT: &str = "An agent";
/// The longest agent name kept, in characters.
pub const AGENT_NAME_LENGTH: usize = 60;

/// AGT-037 — the name an agent gave for itself, as the window and the log may show it. It is
/// text this program did not write: control characters and the invisible characters that
/// reorder or hide text are removed, the length is bounded, and an empty name is replaced.
pub fn shown_name(given: &str) -> String {
    let name: String = given
        .chars()
        .filter(|character| !character.is_control() && !is_invisible(*character))
        .take(AGENT_NAME_LENGTH)
        .collect();
    let name = name.trim();
    if name.is_empty() {
        UNNAMED_AGENT.to_string()
    } else {
        name.to_string()
    }
}

/// Zero-width, directional and other formatting characters: they show nothing and can
/// make a name read as another.
fn is_invisible(character: char) -> bool {
    matches!(
        character,
        '\u{00AD}'
            | '\u{061C}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
    )
}

/// Writes `frame` as one line.
pub async fn write_frame<W, T>(writer: &mut W, frame: &T) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let mut line = serde_json::to_vec(frame).map_err(std::io::Error::other)?;
    line.push(b'\n');
    writer.write_all(&line).await?;
    writer.flush().await
}

/// Reads the next line as a frame: `Ok(None)` when the other side closed, an error when
/// the line is too long or is not such a frame.
pub async fn read_frame<R, T>(reader: &mut R) -> std::io::Result<Option<T>>
where
    R: AsyncBufRead + Unpin,
    T: for<'de> Deserialize<'de>,
{
    match read_line(reader).await? {
        None => Ok(None),
        Some(line) => serde_json::from_str(&line)
            .map(Some)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
    }
}

/// Reads the next line, at most `MAX_FRAME_BYTES` long: `Ok(None)` at the end of the input.
pub async fn read_line<R>(reader: &mut R) -> std::io::Result<Option<String>>
where
    R: AsyncBufRead + Unpin,
{
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            break;
        }
        let end = available.iter().position(|byte| *byte == b'\n');
        let taken = end.map_or(available.len(), |position| position + 1);
        line.extend_from_slice(available.get(..taken).unwrap_or(available));
        reader.consume(taken);
        if line.len() > MAX_FRAME_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "the line is too long",
            ));
        }
        if end.is_some() {
            break;
        }
    }
    if line.is_empty() {
        return Ok(None);
    }
    String::from_utf8(line)
        .map(|text| Some(text.trim_end_matches(['\n', '\r']).to_string()))
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    // AGT-020 — a request and an answer each travel as one line and come back as written.
    #[tokio::test]
    async fn agt_020_a_frame_is_one_line_read_back_as_written() {
        let call = Request::Call {
            tool: "list_accounts".to_string(),
            arguments: serde_json::json!({ "note": "two\nlines" }),
        };
        let mut sent = Vec::new();
        write_frame(&mut sent, &call).await.expect("written");
        write_frame(&mut sent, &Answer::Granted)
            .await
            .expect("written");

        assert_eq!(sent.iter().filter(|byte| **byte == b'\n').count(), 2);
        let mut reader = BufReader::new(sent.as_slice());
        assert_eq!(
            read_frame::<_, Request>(&mut reader).await.expect("read"),
            Some(call)
        );
        assert_eq!(
            read_frame::<_, Answer>(&mut reader).await.expect("read"),
            Some(Answer::Granted)
        );
        assert_eq!(
            read_frame::<_, Answer>(&mut reader).await.expect("end"),
            None
        );
    }

    // AGT-020 — what is not a frame, or is too long to be one, ends the exchange with an error.
    #[tokio::test]
    async fn agt_020_a_line_that_is_no_frame_or_is_too_long_is_an_error() {
        let mut not_a_frame = BufReader::new(&b"{\"type\":\"delete_everything\"}\n"[..]);
        assert!(read_frame::<_, Request>(&mut not_a_frame).await.is_err());

        let endless = vec![b'a'; MAX_FRAME_BYTES + 2];
        let mut reader = BufReader::new(endless.as_slice());
        assert!(read_line(&mut reader).await.is_err());

        let mut not_text = BufReader::new(&[0xff_u8, 0xfe, b'\n'][..]);
        assert!(read_line(&mut not_text).await.is_err());
    }
}
