type Token = number | "+" | "-" | "*" | "/" | "(" | ")";

/**
 * Tokenises an arithmetic expression into numbers and the operators
 * `+ - * / ( )`. The decimal separator is "." (the HTML number-input
 * convention the calc fields replace). Returns null on any unrecognised
 * character or malformed number (e.g. two decimal points).
 */
function tokenize(expr: string): Token[] | null {
  const tokens: Token[] = [];
  const s = expr.trim();
  let i = 0;
  while (i < s.length) {
    const ch = s.charAt(i);
    if (ch === " " || ch === "\t") {
      i++;
      continue;
    }
    if (ch === "+" || ch === "-" || ch === "*" || ch === "/" || ch === "(" || ch === ")") {
      tokens.push(ch);
      i++;
      continue;
    }
    if ((ch >= "0" && ch <= "9") || ch === ".") {
      let numStr = "";
      let d = s.charAt(i);
      while (i < s.length && ((d >= "0" && d <= "9") || d === ".")) {
        numStr += d;
        i++;
        d = s.charAt(i);
      }
      if ((numStr.match(/\./g) ?? []).length > 1) return null; // "1.2.3"
      if (numStr === ".") return null; // a lone separator
      const n = Number(numStr);
      if (Number.isNaN(n)) return null;
      tokens.push(n);
      continue;
    }
    return null; // unknown character
  }
  return tokens;
}

/**
 * Evaluates a basic arithmetic expression (`+ - * / ( )` plus unary `±`) over
 * decimal numbers and returns the numeric result, or null when the input is
 * empty, malformed, divides by zero, or overflows to a non-finite value.
 *
 * Callers treat null as "not a computable expression" and leave the raw text
 * in place. This is a hand-written recursive-descent parser over a fixed token
 * set — no identifiers, no functions, no `eval`.
 */
export function evaluateArithmetic(expr: string): number | null {
  const tokens = tokenize(expr);
  if (tokens === null || tokens.length === 0) return null;

  let pos = 0;
  const peek = (): Token | undefined => tokens[pos];
  const next = (): Token | undefined => tokens[pos++];

  // factor := number | "(" expr ")" | ("+" | "-") factor
  function parseFactor(): number | null {
    const tok = peek();
    if (tok === "+" || tok === "-") {
      next();
      const operand = parseFactor();
      if (operand === null) return null;
      return tok === "-" ? -operand : operand;
    }
    if (tok === "(") {
      next();
      const inner = parseExpr();
      if (inner === null) return null;
      if (next() !== ")") return null;
      return inner;
    }
    if (typeof tok === "number") {
      next();
      return tok;
    }
    return null;
  }

  // term := factor (("*" | "/") factor)*
  function parseTerm(): number | null {
    let left = parseFactor();
    if (left === null) return null;
    while (peek() === "*" || peek() === "/") {
      const op = next();
      const right = parseFactor();
      if (right === null) return null;
      if (op === "/") {
        if (right === 0) return null;
        left = left / right;
      } else {
        left = left * right;
      }
    }
    return left;
  }

  // expr := term (("+" | "-") term)*
  function parseExpr(): number | null {
    let left = parseTerm();
    if (left === null) return null;
    while (peek() === "+" || peek() === "-") {
      const op = next();
      const right = parseTerm();
      if (right === null) return null;
      left = op === "+" ? left + right : left - right;
    }
    return left;
  }

  const result = parseExpr();
  if (result === null || pos !== tokens.length) return null; // trailing tokens → malformed
  if (!Number.isFinite(result)) return null;
  return result;
}
