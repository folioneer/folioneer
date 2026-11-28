//! AGT-077 — the rules the application applies to a holding's cost, its realized result, its
//! income and its value, and to a return, each naming the spec rules it states, with an example the
//! application computes as stated where the tools can show one. They describe the arithmetic
//! only: no tax is computed and no country's tax treatment is named.

use serde_json::{json, Value};

/// What `describe_rules` answers: the rules, and what they are and are not.
pub(crate) fn described() -> Value {
    json!({
        "status": "described",
        "scope": "How the application computes a holding's cost, its realized result, its income and its value, and a return. It computes no tax and states no country's tax treatment: a tax computation is yours to make from these figures and rules. Each example records on an imaginary account named Example, in EUR, holding an imaginary asset ACME, then reads the figure the last call answers; never run it on the portfolio.",
        "rules": rules(),
    })
}

/// One recording or read of an example: its tool and its arguments.
fn call(tool: &str, arguments: Value) -> Value {
    json!({ "tool": tool, "arguments": arguments })
}

/// An example: the account's country (which sets its method, CBM-011) and currency, the
/// asset's currency, the calls in order, and the figure the last call answers at `at` (a
/// JSON pointer).
fn example(country: &str, asset_currency: &str, calls: Vec<Value>, at: &str, value: &str) -> Value {
    json!({
        "account_country": country,
        "account_currency": "EUR",
        "asset_currency": asset_currency,
        "calls": calls,
        "answers": { "at": at, "value": value },
    })
}

/// An example whose account has management fees turned on (FEE-075), named as `add_account`
/// takes it (#944).
fn with_management_fees(mut example: Value) -> Value {
    if let Some(fields) = example.as_object_mut() {
        fields.insert("management_fees".to_string(), Value::Bool(true));
    }
    example
}

fn trade(tool: &str, date: &str, quantity: &str, price: &str) -> Value {
    call(
        tool,
        json!({ "account": "Example", "asset": "ACME", "date": date, "quantity": quantity, "price": price }),
    )
}

fn lots_of(account: &str) -> Value {
    call("list_lots", json!({ "account": account, "asset": "ACME" }))
}

fn two_purchases_then_a_sale() -> Vec<Value> {
    vec![
        trade("record_purchase", "2024-02-01", "10", "100"),
        trade("record_purchase", "2024-03-01", "10", "200"),
        trade("record_sale", "2024-04-01", "15", "250"),
    ]
}

/// The rules, in the order an agent computing a result needs them. Every figure is text, as
/// an agent reads and writes one (AGT-014).
fn rules() -> Vec<Value> {
    vec![
        json!({
            "rules": ["CBM-010", "CBM-011", "CBM-011A", "CBM-030"],
            "topic": "Cost basis method",
            "statement": "Each account has one cost basis method, given by list_accounts and list_lots as cost_basis_method: WeightedAverage or Fifo. A new account takes a default the application sets from its country — the country of its wrapper when the wrapper belongs to a country's list, otherwise the account's: WeightedAverage for France and Italy, Fifo for Germany, Spain and Poland, WeightedAverage elsewhere — unless the owner chose one at creation. These defaults are the application's, not a statement of any country's rule. The owner may change the method, which recomputes the account's history.",
            "example": example("DE", "EUR", vec![lots_of("Example")], "/cost_basis_method", "Fifo"),
        }),
        json!({
            "rules": ["TRX-030", "SEL-024"],
            "topic": "A sale under the weighted average",
            "statement": "The average price is the holding's cost divided by its quantity, fees included. A sale takes its units at the average price just before it; its realized result is its proceeds minus that cost.",
            "example": example("FR", "EUR", two_purchases_then_a_sale(), "/transaction/realized_pnl", "1500"),
        }),
        json!({
            "rules": ["CBM-020", "CBM-022"],
            "topic": "A sale under FIFO",
            "statement": "Each purchase, opening balance, free shares, interest on an asset and transfer in brings lots in, dated and at their cost. A sale, a write-off and a transfer out take the oldest lots first: a lot taken whole leaves with its whole cost, a lot taken in part with its cost in proportion, rounded down. A sale's realized result is its proceeds minus the cost of the lots it took. list_lots reads a holding's lots.",
            "example": example("DE", "EUR", two_purchases_then_a_sale(), "/transaction/realized_pnl", "1750"),
        }),
        json!({
            "rules": ["TRX-026", "TRX-021"],
            "topic": "A purchase's cost: fees and the exchange rate",
            "statement": "A purchase costs its quantity times its unit price in the asset's currency, converted into the account's currency at the rate recorded with the purchase, each step rounded down, plus its fees in the account's currency. Fees are part of the cost.",
            "example": example("FR", "USD", vec![call(
                "record_purchase",
                json!({ "account": "Example", "asset": "ACME", "date": "2024-02-01", "quantity": "10", "price": "100", "rate": "0.9", "fees": "5" }),
            )], "/transaction/total_amount", "905"),
        }),
        json!({
            "rules": ["SEL-023", "SEL-024"],
            "topic": "A sale's proceeds",
            "statement": "A sale's proceeds are its quantity times its unit price, converted at its rate, each step rounded down, minus its fees: fees reduce the proceeds, and so the realized result.",
            "example": example("FR", "EUR", vec![
                call("record_purchase", json!({ "account": "Example", "asset": "ACME", "date": "2024-02-01", "quantity": "10", "price": "100", "fees": "5" })),
                call("record_sale", json!({ "account": "Example", "asset": "ACME", "date": "2024-03-01", "quantity": "10", "price": "120", "fees": "5" })),
            ], "/transaction/realized_pnl", "190"),
        }),
        json!({
            "rules": ["TRX-060", "SEL-050"],
            "topic": "An all-in total",
            "statement": "When a purchase or a sale is recorded with the broker's all-in total in the account's currency, that total is kept as it is and the unit price is derived from it, rounded half away from zero.",
            "example": example("FR", "EUR", vec![call(
                "record_purchase",
                json!({ "account": "Example", "asset": "ACME", "date": "2024-02-01", "quantity": "3", "total": "1000" }),
            )], "/transaction/unit_price", "333.333333"),
        }),
        json!({
            "rules": ["AGT-014", "TRX-024B", "SEL-024", "CBM-027"],
            "topic": "Rounding",
            "statement": "Amounts are exact to six decimals and quantities of an asset to eight. An average price is rounded down, and the cost a removal takes is rounded down: a realized result keeps what that rounding leaves.",
            "example": example("FR", "EUR", vec![
                call("record_opening_balance", json!({ "account": "Example", "asset": "ACME", "date": "2024-02-01", "quantity": "3", "total_cost": "100" })),
                trade("record_sale", "2024-03-01", "1", "50"),
            ], "/transaction/realized_pnl", "16.666667"),
        }),
        json!({
            "rules": ["WOF-021"],
            "topic": "A write-off",
            "statement": "Units written off — lost, dust, a network fee — leave at their cost and realize that cost as a loss, with no proceeds.",
            "example": example("FR", "EUR", vec![
                trade("record_purchase", "2024-02-01", "10", "100"),
                call("record_write_off", json!({ "account": "Example", "asset": "ACME", "date": "2024-03-01", "quantity": "4", "reason": "Lost" })),
            ], "/transaction/realized_pnl", "-400"),
        }),
        json!({
            "rules": ["FSD-022", "INT-024"],
            "topic": "Free shares and interest in units",
            "statement": "Free shares, and interest paid in units of an asset, come in at no cost: the holding's cost does not change and its average price falls, so their value surfaces in the gain — unrealized while held, realized as units are sold. Under FIFO they are a lot at no cost.",
            "example": example("FR", "EUR", vec![
                trade("record_purchase", "2024-02-01", "10", "100"),
                call("record_free_shares", json!({ "account": "Example", "asset": "ACME", "date": "2024-03-01", "quantity": "10" })),
                trade("record_sale", "2024-04-01", "10", "100"),
            ], "/transaction/realized_pnl", "500"),
        }),
        json!({
            "rules": ["DIV-022", "DIV-024"],
            "topic": "Dividends",
            "statement": "A dividend is recorded as the net amount received, in the asset's currency, converted into the account's at its rate and credited to cash. It is income kept apart from realized gains: it changes neither the holding's cost nor its realized result.",
            "example": example("FR", "USD", vec![
                call("record_purchase", json!({ "account": "Example", "asset": "ACME", "date": "2024-02-01", "quantity": "10", "price": "100", "rate": "0.9" })),
                call("record_dividend", json!({ "account": "Example", "asset": "ACME", "date": "2024-03-01", "amount": "10", "rate": "0.9" })),
            ], "/transaction/total_amount", "9"),
        }),
        json!({
            "rules": ["INT-023", "PRF-076"],
            "topic": "Interest on cash, and what is earned",
            "statement": "Interest on the account's cash is credited to cash, as its quantity: no deposit is recorded. What the account earns in kind or as interest — free shares, interest — is no money brought in: performance counts it as a gain, never as a contribution.",
            "example": example("FR", "EUR", vec![call(
                "record_interest",
                json!({ "account": "Example", "date": "2024-03-01", "quantity": "12" }),
            )], "/transaction/quantity", "12"),
        }),
        json!({
            "rules": ["FEE-023", "CBM-023"],
            "topic": "A management fee taken in units",
            "statement": "A fee taken in units lowers the quantity and keeps the cost: nothing is realized and the average price rises. Under FIFO every lot loses the same share of its units and keeps its cost.",
            "example": with_management_fees(example("DE", "EUR", vec![
                trade("record_purchase", "2024-02-01", "10", "100"),
                call("record_management_fee", json!({ "account": "Example", "asset": "ACME", "date": "2024-03-01", "resulting_quantity": "9" })),
                lots_of("Example"),
            ], "/lots/0/cost", "1000")),
        }),
        json!({
            "rules": ["TRF-020", "TRF-021", "TRF-023", "CBM-024"],
            "topic": "A transfer between two accounts",
            "statement": "A transfer moves units at their cost and realizes nothing; no cash moves. A fee in kind it leaves on the way is a write-off. Under FIFO the receiving account gets the lots the sending account gave up, each with the date and cost it was acquired at; from an account under the weighted average, one lot at the cost moved, dated the transfer. The example's receiving account, Other, is like Example.",
            "example": example("DE", "EUR", vec![
                trade("record_purchase", "2024-02-01", "10", "100"),
                call("record_transfer", json!({ "from": "Example", "to": "Other", "asset": "ACME", "date": "2024-04-01", "quantity": "5" })),
                lots_of("Other"),
            ], "/lots/0/acquired_on", "2024-02-01"),
        }),
        json!({
            "rules": ["FXR-034", "FXR-035", "PRF-022", "PRF-022A"],
            "topic": "Valuing a holding",
            "statement": "A holding is valued at the latest recorded price on or before the date asked, converted at the latest recorded rate on or before that date; with no recorded price it is worth its cost; a priced holding in another currency with no usable rate has no unrealized gain and counts 0 in the totals.",
            "example": example("FR", "EUR", vec![
                trade("record_purchase", "2024-02-01", "10", "100"),
                call("record_price", json!({ "asset": "ACME", "date": "2024-03-01", "price": "150" })),
                call("list_holdings", json!({ "account": "Example" })),
            ], "/total_unrealized_pnl", "500"),
        }),
        json!({
            "rules": ["PRF-030", "PRF-031", "PRF-032", "PRF-033", "PRF-034", "PRF-035", "PRF-038", "PRF-086", "PRF-087"],
            "topic": "A return over a period",
            "statement": "A return is a gain net of the money that came in or went out, over a base that weights each flow by the share of the period left after it (Simple Dietz): the gain is the end value, less the start value, less the deposits net of withdrawals over the period — an opening balance or a transfer in counted at its market value on its date for period over period and year to date, at its recorded cost since inception; the base is the start value plus each of those flows weighted by the share of the period left after it, and the return is the gain over the base, in percent. Purchases and sales move nothing in or out. Period over period starts from the previous period's end; year to date from the previous 31 December; since inception from nothing, against the money put in. Free shares, interest and price moves are gain, never money brought in (PRF-076). A year row also gives the annualized yield: the since-inception return as a yearly rate over the years since the first transaction, given as it is under a year. Since-inception and annualized are absent when nothing was invested — an opening balance at no cost, or more taken out than put in — and the answer says which. The example's account starts, like every example's, with 100000 deposited on 2024-01-02.",
            "example": example("FR", "EUR", vec![
                trade("record_purchase", "2024-02-01", "10", "100"),
                call("record_price", json!({ "asset": "ACME", "date": "2024-12-31", "price": "110" })),
                call("get_performance", json!({ "account": "Example", "year": 2024 })),
            ], "/yearly/0/since_inception/pct", "0.1"),
        }),
    ]
}
