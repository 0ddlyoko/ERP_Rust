/*! Trame v0.2.0 | LGPL v3 | https://github.com/0ddlyoko/Trame */
var __defProp = Object.defineProperty;
var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
var __publicField = (obj, key, value) => __defNormalProp(obj, typeof key !== "symbol" ? key + "" : key, value);

// src/compiler/xml.ts
var TemplateSyntaxError = class extends Error {
  constructor(message, source, index) {
    const before = source.slice(0, index);
    const line = before.split("\n").length;
    const column = index - before.lastIndexOf("\n");
    super(`${message} (ligne ${line}, colonne ${column})`);
    this.name = "TemplateSyntaxError";
  }
};
var NAMED_ENTITIES = {
  lt: "<",
  gt: ">",
  amp: "&",
  quot: '"',
  apos: "'",
  nbsp: "\xA0",
  copy: "\xA9",
  reg: "\xAE",
  euro: "\u20AC",
  hellip: "\u2026",
  mdash: "\u2014",
  ndash: "\u2013",
  laquo: "\xAB",
  raquo: "\xBB",
  times: "\xD7",
  middot: "\xB7",
  bull: "\u2022",
  deg: "\xB0"
};
function decodeEntities(text) {
  if (text.indexOf("&") === -1) {
    return text;
  }
  return text.replace(/&(#x[0-9a-fA-F]+|#[0-9]+|[a-zA-Z]+);/g, (match, entity) => {
    if (entity[0] === "#") {
      const code = entity[1] === "x" ? parseInt(entity.slice(2), 16) : parseInt(entity.slice(1), 10);
      return String.fromCodePoint(code);
    }
    const value = NAMED_ENTITIES[entity];
    return value === void 0 ? match : value;
  });
}
var NAME_START = /[A-Za-z_:]/;
var NAME_CHAR = /[\w:.\-@]/;
function parseXML(source, origin) {
  const roots = [];
  const stack = [];
  let i = 0;
  const n = source.length;
  let lineIndex = 0;
  let line = 1;
  const lineAt = (index) => {
    for (; lineIndex < index; lineIndex++) {
      if (source.charCodeAt(lineIndex) === 10) {
        line++;
      }
    }
    return line;
  };
  const current = () => stack.length ? stack[stack.length - 1].children : roots;
  const parentEl = () => stack.length ? stack[stack.length - 1] : null;
  const fail = (message, at = i) => {
    throw new TemplateSyntaxError(message, source, at);
  };
  const pushText = (raw) => {
    if (raw.length === 0) {
      return;
    }
    const list = current();
    const last = list[list.length - 1];
    const value = decodeEntities(raw);
    if (last !== void 0 && last.type === "text") {
      last.value += value;
    } else {
      list.push({ type: "text", value, parent: parentEl(), line: lineAt(i), origin });
    }
  };
  const readName = () => {
    const start = i;
    if (i >= n || !NAME_START.test(source[i])) {
      fail("Nom attendu");
    }
    i++;
    while (i < n && NAME_CHAR.test(source[i])) {
      i++;
    }
    return source.slice(start, i);
  };
  const skipSpaces = () => {
    while (i < n && /\s/.test(source[i])) {
      i++;
    }
  };
  while (i < n) {
    const lt = source.indexOf("<", i);
    if (lt === -1) {
      pushText(source.slice(i));
      break;
    }
    pushText(source.slice(i, lt));
    i = lt;
    if (source.startsWith("<!--", i)) {
      const end = source.indexOf("-->", i + 4);
      if (end === -1) {
        fail("Commentaire non ferm\xE9");
      }
      i = end + 3;
      continue;
    }
    if (source.startsWith("<![CDATA[", i)) {
      const end = source.indexOf("]]>", i + 9);
      if (end === -1) {
        fail("CDATA non ferm\xE9");
      }
      const list = current();
      list.push({ type: "text", value: source.slice(i + 9, end), parent: parentEl() });
      i = end + 3;
      continue;
    }
    if (source.startsWith("<?", i)) {
      const end = source.indexOf("?>", i + 2);
      if (end === -1) {
        fail("Instruction de traitement non ferm\xE9e");
      }
      i = end + 2;
      continue;
    }
    if (source.startsWith("<!", i)) {
      const end = source.indexOf(">", i + 2);
      i = end === -1 ? n : end + 1;
      continue;
    }
    if (source[i + 1] === "/") {
      const start = i;
      i += 2;
      const tag2 = readName();
      skipSpaces();
      if (source[i] !== ">") {
        fail(`">" attendu pour fermer </${tag2}>`);
      }
      i++;
      const open = stack.pop();
      if (open === void 0) {
        fail(`Balise fermante </${tag2}> sans balise ouvrante`, start);
      } else if (open.tag !== tag2) {
        fail(`Balise fermante </${tag2}> inattendue : <${open.tag}> est encore ouverte`, start);
      }
      continue;
    }
    i++;
    const tag = readName();
    const el = { type: "element", tag, attrs: [], children: [], parent: parentEl(), line: lineAt(i), origin };
    for (; ; ) {
      skipSpaces();
      if (i >= n) {
        fail(`Balise <${tag}> non termin\xE9e`);
      }
      const c = source[i];
      if (c === "/") {
        if (source[i + 1] !== ">") {
          fail('"/>" attendu');
        }
        i += 2;
        current().push(el);
        break;
      }
      if (c === ">") {
        i++;
        current().push(el);
        stack.push(el);
        break;
      }
      const attrStart = i;
      const name = readName();
      skipSpaces();
      let value = "";
      if (source[i] === "=") {
        i++;
        skipSpaces();
        const quote = source[i];
        if (quote !== '"' && quote !== "'") {
          fail(`Valeur de l'attribut "${name}" : guillemets attendus`);
        }
        const end = source.indexOf(quote, i + 1);
        if (end === -1) {
          fail(`Valeur de l'attribut "${name}" non ferm\xE9e`);
        }
        value = decodeEntities(source.slice(i + 1, end));
        i = end + 1;
      }
      if (el.attrs.some((a) => a.name === name)) {
        fail(`Attribut "${name}" en double`, attrStart);
      }
      el.attrs.push({ name, value });
    }
  }
  if (stack.length) {
    const open = stack[stack.length - 1];
    throw new TemplateSyntaxError(`Balise <${open.tag}> non ferm\xE9e`, source, n);
  }
  return roots;
}
function getAttr(el, name) {
  for (const attr of el.attrs) {
    if (attr.name === name) {
      return attr.value;
    }
  }
  return void 0;
}
function setAttr(el, name, value) {
  for (const attr of el.attrs) {
    if (attr.name === name) {
      attr.value = value;
      return;
    }
  }
  el.attrs.push({ name, value });
}
function removeAttr(el, name) {
  const index = el.attrs.findIndex((a) => a.name === name);
  if (index !== -1) {
    el.attrs.splice(index, 1);
  }
}
function cloneNode(node, parent = null) {
  if (node.type === "text") {
    return { type: "text", value: node.value, parent, line: node.line, origin: node.origin };
  }
  const el = {
    type: "element",
    tag: node.tag,
    attrs: node.attrs.map((a) => ({ ...a })),
    children: [],
    parent,
    line: node.line,
    origin: node.origin
  };
  el.children = node.children.map((c) => cloneNode(c, el));
  return el;
}

// src/compiler/expression.ts
function defaultMacro(scope, name, args) {
  const member = scope.free(name);
  return `(typeof ${member} === "function" ? ${member}(${args}) : $h.${name}(() => (${args})))`;
}
var KEYWORDS = /* @__PURE__ */ new Set([
  "true",
  "false",
  "null",
  "undefined",
  "typeof",
  "instanceof",
  "in",
  "of",
  "new",
  "void",
  "delete",
  "NaN",
  "Infinity",
  "async",
  "await",
  "function",
  "return",
  "if",
  "else",
  "let",
  "const",
  "var",
  "for",
  "while",
  "do",
  "break",
  "continue",
  "switch",
  "case",
  "default",
  "throw",
  "try",
  "catch",
  "finally",
  "yield",
  "class",
  "super"
]);
var RESERVED = /* @__PURE__ */ new Set([
  "break",
  "case",
  "catch",
  "class",
  "const",
  "continue",
  "debugger",
  "default",
  "delete",
  "do",
  "else",
  "enum",
  "export",
  "extends",
  "false",
  "finally",
  "for",
  "function",
  "if",
  "import",
  "in",
  "instanceof",
  "let",
  "new",
  "null",
  "return",
  "super",
  "switch",
  "this",
  "throw",
  "true",
  "try",
  "typeof",
  "var",
  "void",
  "while",
  "with",
  "yield",
  "await"
]);
var LITERALS = /* @__PURE__ */ new Set(["true", "false", "null", "undefined", "NaN", "Infinity"]);
var GLOBALS = /* @__PURE__ */ new Set([
  "Math",
  "JSON",
  "Date",
  "Number",
  "String",
  "Boolean",
  "Object",
  "Array",
  "Symbol",
  "RegExp",
  "Error",
  "Promise",
  "Map",
  "Set",
  "WeakMap",
  "WeakSet",
  "Intl",
  "parseInt",
  "parseFloat",
  "isNaN",
  "isFinite",
  "encodeURIComponent",
  "decodeURIComponent",
  "encodeURI",
  "decodeURI",
  "console",
  "window",
  "document",
  "navigator",
  "location",
  "setTimeout",
  "clearTimeout",
  "setInterval",
  "clearInterval",
  "requestAnimationFrame",
  "BigInt"
]);
var MACROS = /* @__PURE__ */ new Set(["loading", "error", "refresh"]);
var HELPERS = /* @__PURE__ */ new Set(["markup", "_t"]);
var PUNCTUATORS = [
  ">>>=",
  "...",
  "===",
  "!==",
  "**=",
  "<<=",
  ">>=",
  ">>>",
  "&&=",
  "||=",
  "??=",
  "=>",
  "==",
  "!=",
  "<=",
  ">=",
  "&&",
  "||",
  "??",
  "?.",
  "++",
  "--",
  "+=",
  "-=",
  "*=",
  "/=",
  "%=",
  "&=",
  "|=",
  "^=",
  "**",
  "<<",
  ">>"
];
var ASSIGN_OPS = /* @__PURE__ */ new Set(["=", "+=", "-=", "*=", "/=", "%=", "**=", "<<=", ">>=", ">>>=", "&=", "|=", "^=", "&&=", "||=", "??="]);
var BINARY_OPS = /* @__PURE__ */ new Set(["+", "-", "*", "/", "%", "**", "==", "!=", "===", "!==", "<", ">", "<=", ">=", "<<", ">>", ">>>", "&", "|", "^", "&&", "||", "??"]);
var UNARY_OPS = /* @__PURE__ */ new Set(["!", "~", "+", "-", "++", "--"]);
var UNARY_WORDS = /* @__PURE__ */ new Set(["typeof", "void", "delete", "await"]);
var ID_START = /[A-Za-z_$\u00C0-\uFFFF]/;
var ID_CHAR = /[\w$\u00C0-\uFFFF]/;
var REGEX_AFTER_KEYWORDS = /* @__PURE__ */ new Set(["return", "typeof", "instanceof", "in", "of", "new", "delete", "void", "case", "yield", "await", "throw", "else", "do"]);
var Tokenizer = class {
  constructor(src) {
    __publicField(this, "src", src);
  }
  fail(message, at) {
    throw new TemplateSyntaxError(`${message} dans l'expression \xAB ${this.src} \xBB`, this.src, at);
  }
  /**
   * Découpe la source à partir de `i`. Avec `inTemplate`, s'arrête sur l'accolade fermante qui
   * termine un `${...}` (renvoie sa position dans `end`).
   */
  tokenize(i, inTemplate) {
    var _a, _b, _c;
    const src = this.src;
    const n = src.length;
    const tokens = [];
    let braces = 0;
    const push = (type, value, start, extra) => {
      tokens.push({ type, value, start, index: tokens.length, ...extra });
    };
    while (i < n) {
      const c = src[i];
      const start = i;
      if (/\s/.test(c)) {
        while (i < n && /\s/.test(src[i])) {
          i++;
        }
        push("ws", src.slice(start, i), start);
        continue;
      }
      if (c === "/" && src[i + 1] === "/") {
        while (i < n && src[i] !== "\n") {
          i++;
        }
        push("ws", " ", start);
        continue;
      }
      if (c === "/" && src[i + 1] === "*") {
        const close = src.indexOf("*/", i + 2);
        if (close === -1) {
          this.fail("Commentaire non ferm\xE9", start);
        }
        i = close + 2;
        push("ws", " ", start);
        continue;
      }
      if (ID_START.test(c)) {
        i++;
        while (i < n && ID_CHAR.test(src[i])) {
          i++;
        }
        push("id", src.slice(start, i), start);
        continue;
      }
      if (/[0-9]/.test(c) || c === "." && /[0-9]/.test((_a = src[i + 1]) != null ? _a : "")) {
        const match = /^(?:0[xX][0-9a-fA-F_]+n?|0[bB][01_]+n?|0[oO][0-7_]+n?|(?:\d[\d_]*)?\.?\d[\d_]*(?:[eE][+-]?\d+)?n?)/.exec(src.slice(i));
        const value = match ? match[0] : c;
        i += value.length;
        push("num", value, start);
        continue;
      }
      if (c === '"' || c === "'") {
        i++;
        while (i < n && src[i] !== c) {
          if (src[i] === "\\") {
            i++;
          }
          i++;
        }
        if (i >= n) {
          this.fail("Cha\xEEne non ferm\xE9e", start);
        }
        i++;
        push("str", src.slice(start, i), start);
        continue;
      }
      if (c === "`") {
        const parts = [];
        const subs = [];
        let part = "";
        i++;
        for (; ; ) {
          if (i >= n) {
            this.fail("Template literal non ferm\xE9", start);
          }
          const ch = src[i];
          if (ch === "\\") {
            part += ch + ((_b = src[i + 1]) != null ? _b : "");
            i += 2;
          } else if (ch === "`") {
            i++;
            break;
          } else if (ch === "$" && src[i + 1] === "{") {
            parts.push(part);
            part = "";
            const sub = this.tokenize(i + 2, true);
            subs.push(sub.tokens);
            i = sub.end + 1;
          } else {
            part += ch;
            i++;
          }
        }
        parts.push(part);
        push("tpl", "", start, { parts, subs });
        continue;
      }
      if (c === "/" && regexAllowed(tokens)) {
        let inClass = false;
        i++;
        while (i < n) {
          const ch = src[i];
          if (ch === "\\") {
            i += 2;
            continue;
          }
          if (ch === "[") {
            inClass = true;
          } else if (ch === "]") {
            inClass = false;
          } else if (ch === "/" && !inClass) {
            break;
          } else if (ch === "\n") {
            this.fail("Expression r\xE9guli\xE8re non ferm\xE9e", start);
          }
          i++;
        }
        if (i >= n) {
          this.fail("Expression r\xE9guli\xE8re non ferm\xE9e", start);
        }
        i++;
        while (i < n && /[a-z]/.test(src[i])) {
          i++;
        }
        push("str", src.slice(start, i), start);
        continue;
      }
      let punct = c;
      for (const p of PUNCTUATORS) {
        if (src.startsWith(p, i)) {
          punct = p;
          break;
        }
      }
      if (punct === "?." && /[0-9]/.test((_c = src[i + 2]) != null ? _c : "")) {
        punct = "?";
      }
      if (punct === "{") {
        braces++;
      } else if (punct === "}") {
        if (braces === 0 && inTemplate) {
          return { tokens, end: i };
        }
        braces--;
      }
      i += punct.length;
      push("p", punct, start);
    }
    if (inTemplate) {
      this.fail("Template literal non ferm\xE9", n);
    }
    return { tokens, end: n };
  }
};
function regexAllowed(tokens) {
  for (let j = tokens.length - 1; j >= 0; j--) {
    const t2 = tokens[j];
    if (t2.type === "ws") {
      continue;
    }
    if (t2.type === "p") {
      return t2.value !== ")" && t2.value !== "]" && t2.value !== "}" && t2.value !== "++" && t2.value !== "--";
    }
    return t2.type === "id" && REGEX_AFTER_KEYWORDS.has(t2.value);
  }
  return true;
}
function newScope(parent, fn = false, ownThis = false) {
  return { parent, names: /* @__PURE__ */ new Set(), fn, ownThis };
}
function isBound(scope, name) {
  for (let s = scope; s !== null; s = s.parent) {
    if (s.names.has(name)) {
      return true;
    }
  }
  return false;
}
function hasOwnThis(scope) {
  for (let s = scope; s !== null; s = s.parent) {
    if (s.ownThis) {
      return true;
    }
  }
  return false;
}
var Parser = class _Parser {
  constructor(ctx, tokens, scope) {
    __publicField(this, "ctx", ctx);
    __publicField(this, "scope", scope);
    __publicField(this, "sig");
    __publicField(this, "pos", 0);
    this.sig = tokens.filter((t2) => t2.type !== "ws");
  }
  // --- Outils ---
  peek(offset = 0) {
    return this.sig[this.pos + offset];
  }
  is(value, offset = 0) {
    const t2 = this.sig[this.pos + offset];
    return t2 !== void 0 && (t2.type === "p" || t2.type === "id") && t2.value === value;
  }
  next() {
    const t2 = this.sig[this.pos];
    if (t2 === void 0) {
      this.fail("Fin d'expression inattendue");
    }
    this.pos++;
    return t2;
  }
  expect(value) {
    if (!this.is(value)) {
      const t2 = this.peek();
      this.fail(t2 === void 0 ? `\xAB ${value} \xBB attendu en fin d'expression` : `\xAB ${value} \xBB attendu au lieu de \xAB ${describe(t2)} \xBB`, t2);
    }
    return this.next();
  }
  fail(message, token) {
    var _a;
    const src = this.ctx.src;
    throw new TemplateSyntaxError(`${message} dans l'expression \xAB ${src} \xBB`, src, (_a = token == null ? void 0 : token.start) != null ? _a : src.length);
  }
  within(scope, fn) {
    const prev = this.scope;
    this.scope = scope;
    try {
      return fn();
    } finally {
      this.scope = prev;
    }
  }
  declare(name, kind, token) {
    if (RESERVED.has(name)) {
      this.fail(`\xAB ${name} \xBB ne peut pas \xEAtre un nom de variable`, token);
    }
    let scope = this.scope;
    if (kind === "var") {
      while (!scope.fn && scope.parent !== null) {
        scope = scope.parent;
      }
    }
    scope.names.add(name);
  }
  /** Analyse toute la liste de jetons comme une expression. */
  parseAll() {
    if (this.peek() === void 0) {
      this.fail("Expression vide");
    }
    this.parseExpression(false);
    const extra = this.peek();
    if (extra !== void 0) {
      this.fail(`Jeton inattendu \xAB ${describe(extra)} \xBB`, extra);
    }
  }
  // --- Expressions ---
  parseExpression(noIn) {
    this.parseAssign(noIn);
    while (this.is(",")) {
      this.next();
      this.parseAssign(noIn);
    }
  }
  parseAssign(noIn) {
    var _a;
    const t2 = this.peek();
    if (t2 !== void 0 && t2.type === "id") {
      if (t2.value === "async" && ((_a = this.peek(1)) == null ? void 0 : _a.type) === "id" && this.is("=>", 2)) {
        this.next();
        this.parseArrow(noIn);
        return;
      }
      if (t2.value === "async" && this.is("(", 1) && this.arrowAhead(this.pos + 1)) {
        this.next();
        this.parseArrow(noIn);
        return;
      }
      if (!RESERVED.has(t2.value) && this.is("=>", 1)) {
        this.parseArrow(noIn);
        return;
      }
    }
    if (this.is("(") && this.arrowAhead(this.pos)) {
      this.parseArrow(noIn);
      return;
    }
    this.parseConditional(noIn);
    const op = this.peek();
    if (op !== void 0 && op.type === "p" && ASSIGN_OPS.has(op.value)) {
      this.next();
      this.parseAssign(noIn);
    }
  }
  /** Le "(" en position `i` ouvre-t-il les paramètres d'une fonction fléchée ? */
  arrowAhead(i) {
    let depth = 0;
    for (let j = i; j < this.sig.length; j++) {
      const t2 = this.sig[j];
      if (t2.type !== "p") {
        continue;
      }
      if (t2.value === "(" || t2.value === "[" || t2.value === "{") {
        depth++;
      } else if (t2.value === ")" || t2.value === "]" || t2.value === "}") {
        depth--;
        if (depth === 0) {
          const after = this.sig[j + 1];
          return after !== void 0 && after.type === "p" && after.value === "=>";
        }
      }
    }
    return false;
  }
  parseArrow(noIn) {
    const scope = newScope(this.scope, true, false);
    this.within(scope, () => {
      if (this.is("(")) {
        this.parseParams();
      } else {
        const name = this.next();
        this.declare(name.value, "param", name);
      }
      this.expect("=>");
      if (this.is("{")) {
        this.parseFunctionBody();
      } else {
        this.parseAssign(noIn);
      }
    });
  }
  parseConditional(noIn) {
    this.parseBinary(noIn);
    if (this.is("?")) {
      this.next();
      this.parseAssign(false);
      this.expect(":");
      this.parseAssign(noIn);
    }
  }
  parseBinary(noIn) {
    this.parseUnary();
    for (; ; ) {
      const t2 = this.peek();
      if (t2 === void 0) {
        return;
      }
      const isOp = t2.type === "p" && BINARY_OPS.has(t2.value) || t2.type === "id" && (t2.value === "instanceof" || t2.value === "in" && !noIn);
      if (!isOp) {
        return;
      }
      this.next();
      this.parseUnary();
    }
  }
  parseUnary() {
    const t2 = this.peek();
    if (t2 !== void 0 && (t2.type === "p" && UNARY_OPS.has(t2.value) || t2.type === "id" && UNARY_WORDS.has(t2.value))) {
      this.next();
      this.parseUnary();
      return;
    }
    this.parseLeftHandSide();
    if (this.is("++") || this.is("--")) {
      this.next();
    }
  }
  parseLeftHandSide() {
    let ref = null;
    if (this.is("new")) {
      this.next();
      if (this.is(".")) {
        this.fail("\xAB new.target \xBB n'est pas pris en charge", this.peek());
      }
      this.parseLeftHandSide();
      return;
    }
    ref = this.parsePrimary();
    let first = true;
    for (; ; ) {
      const t2 = this.peek();
      if (t2 === void 0) {
        return;
      }
      if (t2.type === "p" && t2.value === ".") {
        this.next();
        this.propertyName();
      } else if (t2.type === "p" && t2.value === "?.") {
        this.next();
        if (this.is("(")) {
          this.parseArguments();
        } else if (this.is("[")) {
          this.next();
          this.parseExpression(false);
          this.expect("]");
        } else {
          this.propertyName();
        }
      } else if (t2.type === "p" && t2.value === "[") {
        this.next();
        this.parseExpression(false);
        this.expect("]");
      } else if (t2.type === "p" && t2.value === "(") {
        const args = this.parseArguments();
        if (first && ref !== null && MACROS.has(ref.token.value) && args.count > 0) {
          ref.call = { list: args.list, open: args.open, close: args.close };
        }
      } else if (t2.type === "tpl") {
        this.next();
        this.parseTemplate(t2);
      } else {
        return;
      }
      first = false;
    }
  }
  propertyName() {
    const t2 = this.next();
    if (t2.type !== "id") {
      this.fail(`Nom de propri\xE9t\xE9 attendu au lieu de \xAB ${describe(t2)} \xBB`, t2);
    }
  }
  parseArguments() {
    const open = this.expect("(");
    let count = 0;
    while (!this.is(")")) {
      if (this.is("...")) {
        this.next();
      }
      this.parseAssign(false);
      count++;
      if (!this.is(")")) {
        this.expect(",");
      }
    }
    const close = this.expect(")");
    return { list: this.listOf(open), open, close, count };
  }
  /** Liste de jetons d'origine (avec les blancs) à laquelle appartient `token`. */
  listOf(token) {
    return tokenLists.get(token);
  }
  /** Renvoie la référence créée si l'expression primaire est un identifiant. */
  parsePrimary() {
    const t2 = this.peek();
    if (t2 === void 0) {
      this.fail("Fin d'expression inattendue");
    }
    switch (t2.type) {
      case "num":
      case "str":
        this.next();
        return null;
      case "tpl":
        this.next();
        this.parseTemplate(t2);
        return null;
      case "id":
        return this.parseIdentifierPrimary(t2);
      case "p":
        break;
      default:
        this.fail(`Jeton inattendu \xAB ${describe(t2)} \xBB`, t2);
    }
    if (t2.value === "(") {
      this.next();
      this.parseExpression(false);
      this.expect(")");
      return null;
    }
    if (t2.value === "[") {
      this.next();
      while (!this.is("]")) {
        if (this.is(",")) {
          this.next();
          continue;
        }
        if (this.is("...")) {
          this.next();
        }
        this.parseAssign(false);
        if (!this.is("]")) {
          this.expect(",");
        }
      }
      this.expect("]");
      return null;
    }
    if (t2.value === "{") {
      this.parseObjectLiteral();
      return null;
    }
    this.fail(`Jeton inattendu \xAB ${describe(t2)} \xBB`, t2);
  }
  parseIdentifierPrimary(t2) {
    const v = t2.value;
    if (v === "this") {
      this.next();
      this.ctx.thisRefs.push({ token: t2, scope: this.scope });
      return null;
    }
    if (v === "function") {
      this.parseFunction(false);
      return null;
    }
    if (v === "async" && this.is("function", 1)) {
      this.next();
      this.parseFunction(false);
      return null;
    }
    if (LITERALS.has(v)) {
      this.next();
      return null;
    }
    if (v === "class" || v === "super" || v === "import" || v === "yield") {
      this.fail(`\xAB ${v} \xBB n'est pas pris en charge dans les templates`, t2);
    }
    if (RESERVED.has(v)) {
      this.fail(`Jeton inattendu \xAB ${v} \xBB`, t2);
    }
    this.next();
    const ref = { token: t2, scope: this.scope, shorthand: false, call: null };
    this.ctx.refs.push(ref);
    return ref;
  }
  parseTemplate(t2) {
    for (const sub of t2.subs) {
      new _Parser(this.ctx, sub, this.scope).parseAll();
    }
  }
  isPropertyKeyStart(offset) {
    const t2 = this.peek(offset);
    return t2 !== void 0 && (t2.type === "id" || t2.type === "str" || t2.type === "num" || t2.type === "p" && t2.value === "[");
  }
  parseObjectLiteral() {
    this.expect("{");
    while (!this.is("}")) {
      if (this.is("...")) {
        this.next();
        this.parseAssign(false);
      } else {
        let method = false;
        const first = this.peek();
        if (first.type === "id" && (first.value === "get" || first.value === "set" || first.value === "async") && this.isPropertyKeyStart(1)) {
          this.next();
          method = true;
        }
        if (this.is("*")) {
          this.next();
          method = true;
        }
        let keyToken = null;
        if (this.is("[")) {
          this.next();
          this.parseAssign(false);
          this.expect("]");
        } else {
          keyToken = this.next();
          if (keyToken.type !== "id" && keyToken.type !== "str" && keyToken.type !== "num") {
            this.fail(`Cl\xE9 de propri\xE9t\xE9 attendue au lieu de \xAB ${describe(keyToken)} \xBB`, keyToken);
          }
        }
        if (this.is("(")) {
          this.parseMethodRest();
        } else if (method) {
          this.expect("(");
        } else if (this.is(":")) {
          this.next();
          this.parseAssign(false);
        } else {
          if (keyToken === null || keyToken.type !== "id" || RESERVED.has(keyToken.value)) {
            this.fail("\xAB : \xBB attendu apr\xE8s la cl\xE9", this.peek());
          }
          this.ctx.refs.push({ token: keyToken, scope: this.scope, shorthand: true, call: null });
          if (this.is("=")) {
            this.next();
            this.parseAssign(false);
          }
        }
      }
      if (!this.is("}")) {
        this.expect(",");
      }
    }
    this.expect("}");
  }
  /** Paramètres et corps d'une méthode (après sa clé). */
  parseMethodRest() {
    this.within(newScope(this.scope, true, true), () => {
      this.parseParams();
      this.parseFunctionBody();
    });
  }
  /** function [nom](params) { corps }. `declaration` : le nom est déclaré dans la portée courante. */
  parseFunction(declaration) {
    this.expect("function");
    if (this.is("*")) {
      this.next();
    }
    const scope = newScope(this.scope, true, true);
    const name = this.peek();
    if (name !== void 0 && name.type === "id" && !this.is("(")) {
      this.next();
      if (declaration) {
        this.declare(name.value, "let", name);
      } else {
        scope.names.add(name.value);
      }
    } else if (declaration) {
      this.fail("Nom de fonction attendu", name);
    }
    this.within(scope, () => {
      this.parseParams();
      this.parseFunctionBody();
    });
  }
  parseParams() {
    this.expect("(");
    while (!this.is(")")) {
      if (this.is("...")) {
        this.next();
        this.parseBindingTarget("param");
      } else {
        this.parseBindingElement("param");
      }
      if (!this.is(")")) {
        this.expect(",");
      }
    }
    this.expect(")");
  }
  parseBindingElement(kind) {
    this.parseBindingTarget(kind);
    if (this.is("=")) {
      this.next();
      this.parseAssign(false);
    }
  }
  /** Cible d'une déclaration : nom, ou motif de décomposition { a, b: c } / [a, b]. */
  parseBindingTarget(kind) {
    const t2 = this.peek();
    if (t2 === void 0) {
      this.fail("Nom de variable attendu");
    }
    if (t2.type === "id") {
      this.next();
      this.declare(t2.value, kind, t2);
      return;
    }
    if (this.is("[")) {
      this.next();
      while (!this.is("]")) {
        if (this.is(",")) {
          this.next();
          continue;
        }
        if (this.is("...")) {
          this.next();
          this.parseBindingTarget(kind);
        } else {
          this.parseBindingElement(kind);
        }
        if (!this.is("]")) {
          this.expect(",");
        }
      }
      this.expect("]");
      return;
    }
    if (this.is("{")) {
      this.next();
      while (!this.is("}")) {
        if (this.is("...")) {
          this.next();
          this.parseBindingTarget(kind);
        } else if (this.is("[")) {
          this.next();
          this.parseAssign(false);
          this.expect("]");
          this.expect(":");
          this.parseBindingElement(kind);
        } else {
          const key = this.next();
          if (this.is(":")) {
            this.next();
            this.parseBindingElement(kind);
          } else {
            if (key.type !== "id") {
              this.fail(`\xAB : \xBB attendu apr\xE8s \xAB ${describe(key)} \xBB`, this.peek());
            }
            this.declare(key.value, kind, key);
            if (this.is("=")) {
              this.next();
              this.parseAssign(false);
            }
          }
        }
        if (!this.is("}")) {
          this.expect(",");
        }
      }
      this.expect("}");
      return;
    }
    this.fail(`Nom de variable attendu au lieu de \xAB ${describe(t2)} \xBB`, t2);
  }
  // --- Instructions (corps des fonctions) ---
  parseFunctionBody() {
    this.expect("{");
    while (!this.is("}")) {
      this.parseStatement();
    }
    this.expect("}");
  }
  parseBlock(scope = newScope(this.scope)) {
    this.within(scope, () => {
      this.expect("{");
      while (!this.is("}")) {
        this.parseStatement();
      }
      this.expect("}");
    });
  }
  semicolon() {
    if (this.is(";")) {
      this.next();
    }
  }
  parseParenthesized() {
    this.expect("(");
    this.parseExpression(false);
    this.expect(")");
  }
  parseStatement() {
    const t2 = this.peek();
    if (t2 === void 0) {
      this.fail("\xAB } \xBB attendu en fin d'expression");
    }
    if (t2.type === "p") {
      if (t2.value === "{") {
        this.parseBlock();
        return;
      }
      if (t2.value === ";") {
        this.next();
        return;
      }
    }
    if (t2.type === "id") {
      switch (t2.value) {
        case "var":
        case "let":
        case "const":
          this.parseDeclarations(false);
          this.semicolon();
          return;
        case "function":
          this.parseFunction(true);
          return;
        case "async":
          if (this.is("function", 1)) {
            this.next();
            this.parseFunction(true);
            return;
          }
          break;
        case "if":
          this.next();
          this.parseParenthesized();
          this.parseStatement();
          if (this.is("else")) {
            this.next();
            this.parseStatement();
          }
          return;
        case "for":
          this.parseFor();
          return;
        case "while":
          this.next();
          this.parseParenthesized();
          this.parseStatement();
          return;
        case "do":
          this.next();
          this.parseStatement();
          this.expect("while");
          this.parseParenthesized();
          this.semicolon();
          return;
        case "return":
          this.next();
          if (!this.is(";") && !this.is("}") && this.peek() !== void 0) {
            this.parseExpression(false);
          }
          this.semicolon();
          return;
        case "throw":
          this.next();
          this.parseExpression(false);
          this.semicolon();
          return;
        case "break":
        case "continue": {
          this.next();
          const label = this.peek();
          if (label !== void 0 && label.type === "id" && !RESERVED.has(label.value)) {
            this.next();
          }
          this.semicolon();
          return;
        }
        case "try":
          this.next();
          this.parseBlock();
          if (this.is("catch")) {
            this.next();
            const scope = newScope(this.scope);
            this.within(scope, () => {
              if (this.is("(")) {
                this.next();
                this.parseBindingTarget("let");
                this.expect(")");
              }
            });
            this.parseBlock(newScope(scope));
          }
          if (this.is("finally")) {
            this.next();
            this.parseBlock();
          }
          return;
        case "switch":
          this.next();
          this.parseParenthesized();
          this.within(newScope(this.scope), () => {
            this.expect("{");
            while (!this.is("}")) {
              if (this.is("case")) {
                this.next();
                this.parseExpression(false);
                this.expect(":");
              } else if (this.is("default")) {
                this.next();
                this.expect(":");
              } else {
                this.parseStatement();
              }
            }
            this.expect("}");
          });
          return;
        case "class":
          this.fail("\xAB class \xBB n'est pas pris en charge dans les templates", t2);
          break;
        default:
          if (!RESERVED.has(t2.value) && this.is(":", 1)) {
            this.next();
            this.next();
            this.parseStatement();
            return;
          }
      }
    }
    this.parseExpression(false);
    this.semicolon();
  }
  /** var / let / const a = 1, { b } = c. Renvoie true si c'est l'en-tête d'un for...of / for...in. */
  parseDeclarations(inFor) {
    const keyword = this.next().value;
    const kind = keyword === "var" ? "var" : "let";
    for (; ; ) {
      this.parseBindingTarget(kind);
      if (inFor && (this.is("of") || this.is("in"))) {
        return true;
      }
      if (this.is("=")) {
        this.next();
        this.parseAssign(inFor);
      }
      if (!this.is(",")) {
        return false;
      }
      this.next();
    }
  }
  parseFor() {
    this.expect("for");
    if (this.is("await")) {
      this.next();
    }
    this.within(newScope(this.scope), () => {
      this.expect("(");
      let iteration = false;
      if (this.is("var") || this.is("let") || this.is("const")) {
        iteration = this.parseDeclarations(true);
      } else if (!this.is(";")) {
        this.parseExpression(true);
        iteration = this.is("of") || this.is("in");
      }
      if (iteration) {
        this.next();
        this.parseAssign(false);
      } else {
        this.expect(";");
        if (!this.is(";")) {
          this.parseExpression(false);
        }
        this.expect(";");
        if (!this.is(")")) {
          this.parseExpression(false);
        }
      }
      this.expect(")");
      this.parseStatement();
    });
  }
};
var tokenLists = /* @__PURE__ */ new WeakMap();
function registerLists(tokens) {
  for (const t2 of tokens) {
    tokenLists.set(t2, tokens);
    if (t2.subs) {
      for (const sub of t2.subs) {
        registerLists(sub);
      }
    }
  }
}
function describe(t2) {
  return t2.type === "tpl" ? "`...`" : t2.value;
}
function compileExpression(src, scope) {
  const tokens = new Tokenizer(src).tokenize(0, false).tokens;
  registerLists(tokens);
  const ctx = { src, refs: [], thisRefs: [] };
  new Parser(ctx, tokens, newScope(null, true, false)).parseAll();
  const edits = /* @__PURE__ */ new Map();
  const emit = (list, from, to) => {
    let out = "";
    for (let i = from; i <= to; i++) {
      const t2 = list[i];
      const edit = edits.get(t2);
      if (edit !== void 0) {
        out += edit.render();
        i = edit.to.index;
        continue;
      }
      if (t2.type === "tpl") {
        out += "`";
        t2.parts.forEach((part, k) => {
          out += part;
          if (k < t2.subs.length) {
            const sub = t2.subs[k];
            out += "${" + emit(sub, 0, sub.length - 1) + "}";
          }
        });
        out += "`";
      } else {
        out += t2.value;
      }
    }
    return out;
  };
  for (const { token, scope: jsScope } of ctx.thisRefs) {
    if (!hasOwnThis(jsScope)) {
      const code = scope.free("this");
      edits.set(token, { to: token, render: () => code });
    }
  }
  for (const ref of ctx.refs) {
    const name = ref.token.value;
    if (isBound(ref.scope, name) || name === "arguments" && hasOwnThis(ref.scope)) {
      continue;
    }
    const local = scope.resolve(name);
    if (local === void 0 && ref.call !== null) {
      const { list, open, close } = ref.call;
      edits.set(ref.token, {
        to: close,
        render: () => {
          const args = emit(list, open.index + 1, close.index - 1);
          return scope.macro ? scope.macro(name, args) : defaultMacro(scope, name, args);
        }
      });
      continue;
    }
    const code = local != null ? local : HELPERS.has(name) ? `$h.${name}` : GLOBALS.has(name) ? name : scope.free(name);
    if (code === name && !ref.shorthand) {
      continue;
    }
    const text = ref.shorthand ? code === name ? name : `${name}: ${code}` : code;
    edits.set(ref.token, { to: ref.token, render: () => text });
  }
  return emit(tokens, 0, tokens.length - 1);
}
function isSimplePath(src) {
  return /^\s*[A-Za-z_$][\w$]*(\s*\.\s*[A-Za-z_$][\w$]*)*\s*$/.test(src);
}
function isIdentifier(name) {
  return /^[A-Za-z_$][\w$]*$/.test(name) && !KEYWORDS.has(name);
}

// src/compiler/codegen.ts
var PROPERTY_ATTRS = /* @__PURE__ */ new Set(["value", "checked", "selected", "indeterminate", "muted"]);
var Scope = class _Scope {
  constructor(parent, vars, mode, onUse = null) {
    __publicField(this, "parent", parent);
    __publicField(this, "vars", vars);
    __publicField(this, "mode", mode);
    __publicField(this, "onUse", onUse);
  }
  static root(mode) {
    return new _Scope(null, /* @__PURE__ */ new Map(), mode);
  }
  with(name, code) {
    return new _Scope(this, /* @__PURE__ */ new Map([[name, code]]), this.mode);
  }
  withAll(entries, onUse = null) {
    return new _Scope(this, new Map(entries), this.mode, onUse);
  }
  resolve(name) {
    var _a;
    let scope = this;
    while (scope !== null) {
      const code = scope.vars.get(name);
      if (code !== void 0) {
        (_a = scope.onUse) == null ? void 0 : _a.call(scope, name);
        return code;
      }
      scope = scope.parent;
    }
    return void 0;
  }
  free(name) {
    if (name === "this") {
      return "$c";
    }
    if (this.mode === "call") {
      return `(${JSON.stringify(name)} in $p ? $p : $c).${name}`;
    }
    return `$c.${name}`;
  }
};
var CodeGenerator = class {
  constructor(mode, templateName) {
    __publicField(this, "mode", mode);
    __publicField(this, "templateName", templateName);
    __publicField(this, "statics", []);
    __publicField(this, "locations", /* @__PURE__ */ new Map());
    __publicField(this, "counter", 0);
    /** Position du nœud en cours de génération (messages d'erreur de compilation). */
    __publicField(this, "pos");
  }
  uid(prefix) {
    return `${prefix}${++this.counter}`;
  }
  addStatic(code) {
    const name = this.uid("T");
    this.statics.push(`const ${name} = ${code};`);
    return name;
  }
  expr(src, scope) {
    try {
      return compileExpression(src, scope);
    } catch (e) {
      throw new Error(`[trame] ${this.describe(this.pos)} : ${e.message}`);
    }
  }
  /** « template "X", ligne N » (avec l'origine si le nœud vient d'une extension). */
  describe(pos2) {
    let text = `template "${this.templateName}"`;
    if (pos2 == null ? void 0 : pos2.origin) {
      text += ` (${pos2.origin})`;
    }
    if ((pos2 == null ? void 0 : pos2.line) !== void 0) {
      text += `, ligne ${pos2.line}`;
    }
    return text;
  }
  /**
   * Localisation d'une liaison, pour les erreurs d'exécution en mode dev. Déclarée une fois
   * (constante du template) et passée par référence : aucun coût à l'exécution.
   */
  location(pos2, snippet) {
    const clean = snippet.replace(/\s+/g, " ").trim();
    const text = `${this.describe(pos2)} : ${clean.length > 120 ? clean.slice(0, 117) + "..." : clean}`;
    let name = this.locations.get(text);
    if (name === void 0) {
      name = this.uid("L");
      this.locations.set(text, name);
      this.statics.push(`const ${name} = ${JSON.stringify(text)};`);
    }
    return name;
  }
  generate(ast) {
    const scope = Scope.root(this.mode);
    const body = new BlockBuilder(this).build(ast, scope);
    return `"use strict";
${this.statics.join("\n")}
return function render($c, $s, $p) {
${body}
};`;
  }
};
var BlockBuilder = class _BlockBuilder {
  /**
   * @param exclusive  le bloc est construit sous un scope qui lui est propre (ligne, branche, slot...) ;
   *                   faux pour le bloc racine d'un template, construit sous le scope du composant.
   */
  constructor(gen, exclusive = false) {
    __publicField(this, "gen", gen);
    __publicField(this, "exclusive", exclusive);
    __publicField(this, "roots", []);
    __publicField(this, "ops", []);
    /** Composants statiques du bloc : région et index de leur instruction (voir finish). */
    __publicField(this, "components", []);
  }
  /** Construit le corps d'une fonction qui crée le bloc et renvoie ses racines. */
  build(ast, scope) {
    this.addChildren(null, [ast], scope);
    if (this.roots.length === 0) {
      this.roots.push({ spec: { kind: "text", value: "" } });
    }
    return this.finish();
  }
  addChildren(parent, children, scope) {
    let current = scope;
    for (const child of children) {
      current = this.addNode(parent, child, current);
    }
  }
  pushSpec(parent, spec, region) {
    if (parent === null) {
      this.roots.push({ spec, region });
    } else {
      parent.children.push(spec);
    }
  }
  /** Ajoute un point d'ancrage (nœud texte vide) et renvoie son nom de variable. */
  anchor(parent, regionVar) {
    const spec = { kind: "text", value: "", needed: true, varName: this.gen.uid("a") };
    this.pushSpec(parent, spec, regionVar);
    return spec.varName;
  }
  /** Renvoie la portée à utiliser pour les nœuds frères suivants (t-set). */
  addNode(parent, ast, scope) {
    const gen = this.gen;
    if ("pos" in ast && ast.pos !== void 0) {
      gen.pos = ast.pos;
    }
    switch (ast.type) {
      case "text":
        this.pushSpec(parent, { kind: "text", value: ast.value, raw: ast.raw });
        return scope;
      case "textExpr": {
        const spec = { kind: "text", value: "", needed: true, varName: gen.uid("n") };
        this.pushSpec(parent, spec);
        const loc = gen.location(ast.pos, partsSource(ast.parts));
        this.ops.push(`$h.text(${spec.varName}, () => ${this.parts(ast.parts, scope)}, ${loc});`);
        return scope;
      }
      case "multi":
        this.addChildren(parent, ast.children, scope);
        return scope;
      case "element":
        this.addElement(parent, ast, scope);
        return scope;
      case "set": {
        const name = gen.uid("v");
        this.ops.push(`const ${name} = $h.computed(() => (${gen.expr(ast.value, scope)}));`);
        return scope.with(ast.name, `${name}.get()`);
      }
      case "out": {
        const region = gen.uid("r");
        const anchor = this.anchor(parent, region);
        const loc = gen.location(ast.pos, `t-out="${ast.expr}"`);
        this.ops.push(`const ${region} = $h.out(${anchor}, () => (${gen.expr(ast.expr, scope)}), ${loc});`);
        return scope;
      }
      case "if": {
        const region = gen.uid("r");
        const anchor = this.anchor(parent, region);
        let key = "";
        ast.branches.forEach((branch, i) => {
          key += branch.cond === null ? `${i}` : `(${gen.expr(branch.cond, scope)}) ? ${i} : `;
        });
        if (ast.branches[ast.branches.length - 1].cond !== null) {
          key += "-1";
        }
        const loc = gen.location(ast.pos, `t-if="${ast.branches[0].cond}"`);
        const builders = ast.branches.map((b) => this.subBlock(b.body, scope, []));
        this.ops.push(`const ${region} = $h.sw(${anchor}, () => ${key}, [${builders.join(", ")}], ${loc});`);
        return scope;
      }
      case "foreach": {
        const region = gen.uid("r");
        const anchor = this.anchor(parent, region);
        const item = gen.uid("it");
        const index = gen.uid("ix");
        const keyScope = scope.withAll([
          [ast.as, "v"],
          [`${ast.as}_index`, "i"]
        ]);
        const keyFn = ast.key === null ? "null" : `(v, i) => (${gen.expr(ast.key, keyScope)})`;
        let usesIndex = false;
        const rowScope = scope.withAll(
          [
            [ast.as, `${item}.get()`],
            [`${ast.as}_index`, `${index}.get()`]
          ],
          (name) => {
            if (name === `${ast.as}_index`) {
              usesIndex = true;
            }
          }
        );
        const loc = gen.location(ast.pos, `t-foreach="${ast.collection}"` + (ast.key ? ` t-key="${ast.key}"` : ""));
        const collection = gen.expr(ast.collection, scope);
        const rowFn = this.subBlock(ast.body, rowScope, [item, index]);
        this.ops.push(`const ${region} = $h.each(${anchor}, () => (${collection}), ${keyFn}, ${rowFn}, ${loc}, ${usesIndex ? 1 : 0});`);
        return scope;
      }
      case "component": {
        const region = gen.uid("r");
        const anchor = this.anchor(parent, region);
        if (ast.dynamic === null) {
          this.components.push({ region, op: this.ops.length });
        }
        this.ops.push(`const ${region} = ${this.component(ast, anchor, scope)};`);
        return scope;
      }
      case "slot": {
        const region = gen.uid("r");
        const anchor = this.anchor(parent, region);
        const params = ast.params.length ? this.getters(ast.params.map((p) => [p.name, gen.expr(p.expr, scope)])) : "null";
        const fallback = ast.fallback ? this.subBlock(ast.fallback, scope, []) : "null";
        const loc = gen.location(ast.pos, `t-slot="${ast.name}"`);
        this.ops.push(`const ${region} = $h.slot(${anchor}, $s, ${JSON.stringify(ast.name)}, ${params}, ${fallback}, ${loc});`);
        return scope;
      }
      case "keyed": {
        const region = gen.uid("r");
        const anchor = this.anchor(parent, region);
        const loc = gen.location(ast.pos, `t-key="${ast.key}"`);
        const builder = this.subBlock(ast.body, scope, []);
        this.ops.push(`const ${region} = $h.keyed(${anchor}, () => (${gen.expr(ast.key, scope)}), ${builder}, ${loc});`);
        return scope;
      }
      case "call": {
        const region = gen.uid("r");
        const anchor = this.anchor(parent, region);
        const params = this.getters(ast.params.map((p) => [p.name, gen.expr(p.expr, scope)]));
        const loc = gen.location(ast.pos, `t-call="${ast.template}"`);
        this.ops.push(`const ${region} = $h.call(${anchor}, ${JSON.stringify(ast.template)}, $c, $s, ${params}, ${loc});`);
        return scope;
      }
    }
  }
  addElement(parent, ast, scope) {
    const gen = this.gen;
    const spec = { kind: "el", tag: ast.tag, ns: ast.ns, attrs: ast.attrs.slice(), children: [], noTranslate: ast.noTranslate };
    this.pushSpec(parent, spec);
    const own = [];
    const el = () => {
      if (!spec.varName) {
        spec.varName = gen.uid("n");
        spec.needed = true;
      }
      return spec.varName;
    };
    for (const attr of ast.dynAttrs) {
      const value = attr.parts ? this.parts(attr.parts, scope) : `(${gen.expr(attr.expr, scope)})`;
      const name = attr.name;
      const loc = gen.location(ast.pos, attr.parts ? `${name}="${partsSource(attr.parts)}"` : `t-att-${name}="${attr.expr}"`);
      if (name === "class") {
        own.push(`$h.cls(${el()}, () => ${value}, ${loc});`);
      } else if (name === "style") {
        own.push(`$h.style(${el()}, () => ${value}, ${loc});`);
      } else if (PROPERTY_ATTRS.has(name) && ast.ns === "html") {
        own.push(`$h.prop(${el()}, ${JSON.stringify(name)}, () => ${value}, ${loc});`);
      } else {
        own.push(`$h.attr(${el()}, ${JSON.stringify(name)}, () => ${value}, ${loc});`);
      }
    }
    if (ast.attrsSpread) {
      const loc = gen.location(ast.pos, `t-att="${ast.attrsSpread}"`);
      own.push(`$h.attrs(${el()}, () => (${gen.expr(ast.attrsSpread, scope)}), ${loc});`);
    }
    for (const event of ast.events) {
      const mods = event.modifiers.length ? "." + event.modifiers.join(".") : "";
      const loc = gen.location(ast.pos, `t-on-${event.name}${mods}="${event.expr}"`);
      own.push(
        `$h.on(${el()}, ${JSON.stringify(event.name)}, ${this.handler(event.expr, scope)}, ${JSON.stringify(event.modifiers.join(","))}, ${loc});`
      );
    }
    if (ast.ref) {
      own.push(`$h.ref(${el()}, (el) => { ${gen.expr(ast.ref, scope)} = el; });`);
    }
    if (ast.tag === "select") {
      this.addChildren(spec, ast.children, scope);
      this.ops.push(...own);
    } else {
      this.ops.push(...own);
      this.addChildren(spec, ast.children, scope);
    }
  }
  component(ast, anchor, scope) {
    const gen = this.gen;
    const props2 = ast.props.map((p) => [p.name, p.parts ? this.parts(p.parts, scope) : `(${gen.expr(p.expr, scope)})`]);
    let propsCode = this.getters(props2);
    if (ast.spread) {
      propsCode = `$h.props(${propsCode}, () => (${gen.expr(ast.spread, scope)}))`;
    }
    let slotsCode = "null";
    if (ast.slots.length) {
      const entries = ast.slots.map((slot) => {
        const param = gen.uid("sc");
        const slotScope = slot.scope ? scope.with(slot.scope, param) : scope;
        return `${JSON.stringify(slot.name)}: ${this.subBlock(slot.body, slotScope, [param])}`;
      });
      slotsCode = `{ ${entries.join(", ")} }`;
    }
    if (ast.dynamic !== null) {
      const loc2 = gen.location(ast.pos, `t-component="${ast.dynamic}"`);
      return `$h.dyn(${anchor}, $c, () => (${gen.expr(ast.dynamic, scope)}), ${propsCode}, ${slotsCode}, ${loc2})`;
    }
    const loc = gen.location(ast.pos, `<${ast.name}>`);
    return `$h.comp(${anchor}, $c, ${JSON.stringify(ast.name)}, ${propsCode}, ${slotsCode}, ${loc})`;
  }
  handler(src, scope) {
    const code = this.gen.expr(src, scope);
    if (isSimplePath(src)) {
      return `(ev) => ${code}(ev)`;
    }
    return `(ev) => { const r = (${code}); return typeof r === "function" ? r(ev) : r; }`;
  }
  getters(entries) {
    if (entries.length === 0) {
      return "{}";
    }
    return `{ ${entries.map(([name, code]) => `get ${JSON.stringify(name)}() { return ${code}; }`).join(", ")} }`;
  }
  parts(parts, scope) {
    if (parts.length === 1 && parts[0].expr !== void 0) {
      return `$h.s(${this.gen.expr(parts[0].expr, scope)})`;
    }
    return parts.map((p) => p.expr !== void 0 ? `$h.s(${this.gen.expr(p.expr, scope)})` : JSON.stringify(p.text)).join(" + ");
  }
  subBlock(ast, scope, params) {
    const body = new _BlockBuilder(this.gen, true).build(ast, scope);
    return `(${params.join(", ")}) => {
${body}
}`;
  }
  // --- Assemblage ------------------------------------------------------------------------------
  finish() {
    var _a, _b, _c, _d;
    const gen = this.gen;
    if (this.exclusive && this.roots.length === 1) {
      const solo = this.components.find((c) => c.region === this.roots[0].region);
      if (solo !== void 0) {
        this.ops[solo.op] = this.ops[solo.op].replace(/\);$/, ", 1);");
      }
    }
    const nav = [];
    const single = this.roots.length === 1 && this.roots[0].region === void 0;
    const tpl2 = gen.addStatic(`$h.tpl(${JSON.stringify(this.roots.map((r) => encodeSpec(r.spec)))}, ${single ? 0 : 1})`);
    const rootNames = [];
    if (single) {
      const spec = this.roots[0].spec;
      const root = (_a = spec.varName) != null ? _a : gen.uid("f");
      spec.varName = root;
      nav.push(`const ${root} = ${tpl2}();`);
      this.navigate(spec, nav);
      rootNames.push((_b = this.roots[0].region) != null ? _b : root);
    } else {
      const root = gen.uid("f");
      nav.push(`const ${root} = ${tpl2}();`);
      let prev = null;
      for (const entry of this.roots) {
        const spec = entry.spec;
        const name = (_c = spec.varName) != null ? _c : gen.uid("n");
        spec.varName = name;
        nav.push(`const ${name} = ${prev === null ? `${root}.firstChild` : `${prev}.nextSibling`};`);
        prev = name;
        this.navigate(spec, nav);
        rootNames.push((_d = entry.region) != null ? _d : name);
      }
    }
    return `${nav.join("\n")}
${this.ops.join("\n")}
return [${rootNames.join(", ")}];`;
  }
  /** Génère les accès aux nœuds nécessaires dans le sous-arbre de `spec`. */
  navigate(spec, out) {
    if (spec.kind !== "el") {
      return;
    }
    let prevVar = null;
    let prevIndex = -1;
    spec.children.forEach((child, index) => {
      var _a;
      if (!subtreeNeeded(child)) {
        return;
      }
      const name = (_a = child.varName) != null ? _a : this.gen.uid("n");
      child.varName = name;
      const access = prevVar === null ? `${spec.varName}.firstChild${".nextSibling".repeat(index)}` : `${prevVar}${".nextSibling".repeat(index - prevIndex)}`;
      out.push(`const ${name} = ${access};`);
      prevVar = name;
      prevIndex = index;
      this.navigate(child, out);
    });
  }
};
function partsSource(parts) {
  return parts.map((p) => p.expr !== void 0 ? `{{ ${p.expr} }}` : p.text).join("");
}
function subtreeNeeded(spec) {
  if (spec.needed) {
    return true;
  }
  return spec.kind === "el" && spec.children.some(subtreeNeeded);
}
function encodeSpec(spec) {
  if (spec.kind === "text") {
    return spec.raw ? { r: spec.value } : spec.value;
  }
  const ns = spec.ns === "svg" ? 1 : spec.ns === "math" ? 2 : 0;
  const encoded = [spec.tag, spec.attrs.length ? spec.attrs : 0, spec.children.length ? spec.children.map(encodeSpec) : 0, ns];
  if (spec.noTranslate) {
    encoded.push(1);
  }
  return encoded;
}
function generateCode(ast, mode, name) {
  return new CodeGenerator(mode, name).generate(ast);
}

// src/compiler/xpath.ts
function applyExtension(nodes, extension, templateName, origin = "extension") {
  applyOperations(nodes, extensionOperations(parseXML(extension, origin)), templateName);
}
function extensionOperations(nodes) {
  const ops = [];
  for (const node of nodes) {
    if (node.type !== "element") {
      continue;
    }
    if (node.tag === "t" && getAttr(node, "position") === void 0) {
      ops.push(...extensionOperations(node.children));
    } else {
      ops.push(node);
    }
  }
  return ops;
}
function applyOperations(nodes, ops, templateName) {
  const root = { type: "element", tag: "#root", attrs: [], children: nodes, parent: null };
  for (const node of nodes) {
    node.parent = root;
  }
  try {
    for (const op of ops) {
      applyOperation(root, op);
    }
  } catch (e) {
    throw new Error(`[trame] Extension du template "${templateName}" : ${e.message}`);
  } finally {
    for (const node of nodes) {
      node.parent = null;
    }
  }
}
function applyOperation(root, op) {
  var _a;
  const position = (_a = getAttr(op, "position")) != null ? _a : "inside";
  let target;
  if (op.tag === "xpath") {
    const expr = getAttr(op, "expr");
    if (!expr) {
      throw new Error('<xpath> sans attribut "expr"');
    }
    target = evaluate(root, expr)[0];
    if (target === void 0) {
      throw new Error(`aucun \xE9l\xE9ment ne correspond \xE0 "${expr}"`);
    }
  } else {
    const wanted = op.attrs.filter((a) => a.name !== "position");
    target = findFirst(root, (el) => el.tag === op.tag && wanted.every((a) => getAttr(el, a.name) === a.value));
    if (target === void 0) {
      const desc = wanted.map((a) => `${a.name}="${a.value}"`).join(" ");
      throw new Error(`aucun \xE9l\xE9ment <${op.tag} ${desc}> trouv\xE9`);
    }
  }
  const parent = target.parent;
  const content = op.children.map((c) => cloneNode(c));
  switch (position) {
    case "inside":
      for (const node of content) {
        node.parent = target;
        target.children.push(node);
      }
      break;
    case "before":
    case "after": {
      const index = parent.children.indexOf(target) + (position === "after" ? 1 : 0);
      for (const node of content) {
        node.parent = parent;
      }
      parent.children.splice(index, 0, ...content);
      break;
    }
    case "replace": {
      const index = parent.children.indexOf(target);
      const replacement = [];
      for (const node of content) {
        replaceMarker(node, target, replacement);
      }
      for (const node of replacement) {
        node.parent = parent;
      }
      parent.children.splice(index, 1, ...replacement);
      break;
    }
    case "attributes":
      for (const child of op.children) {
        if (child.type !== "element" || child.tag !== "attribute") {
          continue;
        }
        applyAttribute(target, child);
      }
      break;
    default:
      throw new Error(`position inconnue "${position}"`);
  }
}
function replaceMarker(node, original, out) {
  if (node.type === "text" && node.value.trim() === "$0") {
    out.push(original);
    return;
  }
  if (node.type === "element") {
    const children = [];
    for (const child of node.children) {
      replaceMarker(child, original, children);
    }
    node.children = children;
    for (const child of children) {
      child.parent = node;
    }
  }
  out.push(node);
}
function applyAttribute(target, spec) {
  var _a, _b;
  const name = getAttr(spec, "name");
  if (!name) {
    throw new Error('<attribute> sans attribut "name"');
  }
  const add = getAttr(spec, "add");
  const remove = getAttr(spec, "remove");
  if (add !== void 0 || remove !== void 0) {
    const separator = (_a = getAttr(spec, "separator")) != null ? _a : name === "class" ? " " : ",";
    const split = (v) => v.split(separator).map((s) => s.trim()).filter(Boolean);
    let values = split((_b = getAttr(target, name)) != null ? _b : "");
    if (remove !== void 0) {
      const toRemove = new Set(split(remove));
      values = values.filter((v) => !toRemove.has(v));
    }
    if (add !== void 0) {
      for (const v of split(add)) {
        if (!values.includes(v)) {
          values.push(v);
        }
      }
    }
    const joined = values.join(separator === " " ? " " : separator);
    if (joined) {
      setAttr(target, name, joined);
    } else {
      removeAttr(target, name);
    }
    return;
  }
  const text = spec.children.map((c) => c.type === "text" ? c.value : "").join("").trim();
  if (text) {
    setAttr(target, name, text);
  } else {
    removeAttr(target, name);
  }
}
function findFirst(root, predicate) {
  for (const child of root.children) {
    if (child.type === "element") {
      if (predicate(child)) {
        return child;
      }
      const found = findFirst(child, predicate);
      if (found) {
        return found;
      }
    }
  }
  return void 0;
}
function parseSteps(expr) {
  const steps = [];
  let i = 0;
  const n = expr.length;
  let descendant = false;
  if (expr.startsWith("//")) {
    descendant = true;
    i = 2;
  } else if (expr.startsWith("/")) {
    i = 1;
  } else {
    descendant = true;
  }
  while (i < n) {
    let name = "";
    while (i < n && expr[i] !== "/" && expr[i] !== "[") {
      name += expr[i++];
    }
    const predicates = [];
    while (expr[i] === "[") {
      let depth = 0;
      let quote = "";
      const start = i + 1;
      for (; i < n; i++) {
        const c = expr[i];
        if (quote) {
          if (c === quote) {
            quote = "";
          }
        } else if (c === "'" || c === '"') {
          quote = c;
        } else if (c === "[") {
          depth++;
        } else if (c === "]") {
          depth--;
          if (depth === 0) {
            break;
          }
        }
      }
      predicates.push(expr.slice(start, i).trim());
      i++;
    }
    steps.push({ descendant, name: name.trim(), predicates });
    descendant = false;
    if (expr.startsWith("//", i)) {
      descendant = true;
      i += 2;
    } else if (expr[i] === "/") {
      i++;
    }
  }
  return steps;
}
function evaluate(root, expr) {
  let context = [root];
  for (const step of parseSteps(expr)) {
    const next = [];
    for (const node of context) {
      let candidates;
      if (step.name === ".") {
        candidates = [node];
      } else if (step.name === "..") {
        candidates = node.parent ? [node.parent] : [];
      } else {
        const pool = step.descendant ? descendants(node) : elementChildren(node);
        candidates = pool.filter((el) => step.name === "*" || el.tag === step.name);
      }
      for (const predicate of step.predicates) {
        candidates = applyPredicate(candidates, predicate);
      }
      for (const c of candidates) {
        if (!next.includes(c)) {
          next.push(c);
        }
      }
    }
    context = next;
  }
  return context;
}
function elementChildren(node) {
  return node.children.filter((c) => c.type === "element");
}
function descendants(node) {
  const result = [];
  const walk = (el) => {
    for (const child of el.children) {
      if (child.type === "element") {
        result.push(child);
        walk(child);
      }
    }
  };
  walk(node);
  return result;
}
function applyPredicate(nodes, predicate) {
  if (/^\d+$/.test(predicate)) {
    const node = nodes[parseInt(predicate, 10) - 1];
    return node ? [node] : [];
  }
  if (predicate === "last()") {
    return nodes.length ? [nodes[nodes.length - 1]] : [];
  }
  return nodes.filter((el) => testCondition(el, predicate));
}
function unquote(s) {
  const t2 = s.trim();
  return t2.length >= 2 && (t2[0] === "'" || t2[0] === '"') ? t2.slice(1, -1) : t2;
}
function testCondition(el, cond) {
  var _a, _b;
  const parts = cond.split(/\s+and\s+/);
  if (parts.length > 1) {
    return parts.every((p) => testCondition(el, p));
  }
  const c = cond.trim();
  let m = /^@([\w:.\-]+)\s*=\s*(.+)$/.exec(c);
  if (m) {
    return getAttr(el, m[1]) === unquote(m[2]);
  }
  m = /^@([\w:.\-]+)$/.exec(c);
  if (m) {
    return getAttr(el, m[1]) !== void 0;
  }
  m = /^hasclass\((.+)\)$/.exec(c);
  if (m) {
    const classes = ((_a = getAttr(el, "class")) != null ? _a : "").split(/\s+/);
    return m[1].split(",").every((cls) => classes.includes(unquote(cls)));
  }
  m = /^contains\(\s*@([\w:.\-]+)\s*,\s*(.+)\)$/.exec(c);
  if (m) {
    return ((_b = getAttr(el, m[1])) != null ? _b : "").includes(unquote(m[2]));
  }
  m = /^not\((.+)\)$/.exec(c);
  if (m) {
    return !testCondition(el, m[1]);
  }
  throw new Error(`pr\xE9dicat xpath non support\xE9 : [${cond}]`);
}

// src/compiler/files.ts
function copy(nodes, parent = null) {
  return nodes.map((node) => {
    if (node.type === "text") {
      return { ...node, parent };
    }
    const el = { ...node, attrs: node.attrs.map((a) => ({ ...a })), children: [], parent };
    el.children = copy(node.children, el);
    return el;
  });
}
var TemplateLibrary = class {
  constructor() {
    __publicField(this, "definitions", /* @__PURE__ */ new Map());
    __publicField(this, "extensions", []);
  }
  /** Ajoute un fichier de templates. Renvoie les noms définis et les extensions du fichier. */
  addFile(content, path) {
    const defined = [];
    const extensions = [];
    const visit = (nodes) => {
      for (const node of nodes) {
        if (node.type === "text") {
          if (node.value.trim()) {
            throw new Error(`[trame] ${path}, ligne ${node.line} : texte inattendu hors d'un <t t-name> ou <t t-inherit>`);
          }
          continue;
        }
        if (node.tag === "templates") {
          visit(node.children);
          continue;
        }
        const name = getAttr(node, "t-name");
        const inherit = getAttr(node, "t-inherit");
        if (node.tag !== "t" || name === void 0 && inherit === void 0) {
          throw new Error(`[trame] ${path}, ligne ${node.line} : <${node.tag}> inattendu, <t t-name="..."> ou <t t-inherit="..."> attendu`);
        }
        if (name !== void 0) {
          if (this.definitions.has(name)) {
            throw new Error(`[trame] ${path}, ligne ${node.line} : le template "${name}" est d\xE9j\xE0 d\xE9fini (${this.definitions.get(name).origin})`);
          }
          this.definitions.set(name, {
            name,
            origin: path,
            nodes: inherit === void 0 ? node.children : null,
            base: inherit != null ? inherit : null,
            ops: inherit === void 0 ? [] : extensionOperations(node.children)
          });
          defined.push(name);
        } else {
          const ext = { target: inherit, origin: path, ops: extensionOperations(node.children) };
          this.extensions.push(ext);
          extensions.push(ext);
        }
      }
    };
    visit(parseXML(content, path));
    return { defined, extensions };
  }
  /** Extensions dont la cible n'est définie dans aucun fichier. */
  unknownTargets() {
    return this.extensions.filter((e) => !this.definitions.has(e.target)).map(({ target, origin }) => ({ target, origin }));
  }
  has(name) {
    return this.definitions.has(name);
  }
  names() {
    return Array.from(this.definitions.keys());
  }
  /** Arbre final d'un template (base, héritage primaire, extensions), recalculé à chaque appel. */
  resolve(name, seen = []) {
    const def = this.definitions.get(name);
    if (def === void 0) {
      throw new Error(`[trame] Template "${name}" introuvable`);
    }
    if (seen.includes(name)) {
      throw new Error(`[trame] H\xE9ritage circulaire : ${[...seen, name].join(" \u2192 ")}`);
    }
    let nodes;
    if (def.base !== null) {
      nodes = this.resolve(def.base, [...seen, name]);
      applyOperations(nodes, def.ops, name);
    } else {
      nodes = copy(def.nodes);
    }
    for (const ext of this.extensions) {
      if (ext.target === name) {
        applyOperations(nodes, ext.ops, name);
      }
    }
    return nodes;
  }
};

// src/compiler/parser.ts
var BUILTIN_COMPONENTS = /* @__PURE__ */ new Set(["Suspense", "ErrorBoundary", "ErrorHandler", "Portal"]);
var KNOWN_DIRECTIVES = /* @__PURE__ */ new Set([
  "t-if",
  "t-elif",
  "t-else",
  "t-foreach",
  "t-as",
  "t-key",
  "t-set",
  "t-value",
  "t-out",
  "t-att",
  "t-ref",
  "t-component",
  "t-props",
  "t-slot",
  "t-set-slot",
  "t-slot-scope",
  "t-call",
  "t-name",
  "t-translation"
]);
var HANDLED_ELSEWHERE = /* @__PURE__ */ new Set(["t-key", "t-if", "t-elif", "t-else", "t-out", "t-as", "t-foreach", "t-set-slot", "t-slot-scope", "t-translation"]);
var PRESERVE_WHITESPACE = /* @__PURE__ */ new Set(["pre", "textarea"]);
function parseTemplate(nodes) {
  const ctx = new ParseContext();
  const children = ctx.parseChildren(nodes, "html", false);
  return children.length === 1 ? children[0] : { type: "multi", children };
}
function splitInterpolation(text) {
  if (text.indexOf("{{") === -1) {
    return null;
  }
  const parts = [];
  let i = 0;
  while (i < text.length) {
    const start = text.indexOf("{{", i);
    if (start === -1) {
      parts.push({ text: text.slice(i) });
      break;
    }
    if (start > i) {
      parts.push({ text: text.slice(i, start) });
    }
    const end = text.indexOf("}}", start + 2);
    if (end === -1) {
      throw new Error(`[trame] "{{" sans "}}" correspondant dans \xAB ${text} \xBB`);
    }
    const expr = text.slice(start + 2, end).trim();
    if (expr) {
      parts.push({ expr });
    }
    i = end + 2;
  }
  return parts;
}
var ParseContext = class {
  constructor() {
    /** À l'intérieur d'un t-translation="off". */
    __publicField(this, "noTranslate", false);
  }
  parseChildren(nodes, ns, preserve) {
    const result = [];
    let i = 0;
    while (i < nodes.length) {
      const node = nodes[i];
      if (node.type === "text") {
        const ast = this.parseText(node.value, preserve, pos(node));
        if (ast) {
          result.push(ast);
        }
        i++;
        continue;
      }
      if (getAttr(node, "t-elif") !== void 0 || getAttr(node, "t-else") !== void 0) {
        throw new Error(`[trame] <${node.tag}> : t-elif/t-else doit suivre directement un \xE9l\xE9ment t-if ou t-elif`);
      }
      if (getAttr(node, "t-if") !== void 0 && getAttr(node, "t-foreach") === void 0) {
        const branches = [
          { cond: getAttr(node, "t-if"), body: this.parseElement(node, ns, preserve, ["t-if"]) }
        ];
        let j = i + 1;
        for (; ; ) {
          let k = j;
          while (k < nodes.length && nodes[k].type === "text" && !nodes[k].value.trim()) {
            k++;
          }
          const next = nodes[k];
          if (next === void 0 || next.type !== "element") {
            break;
          }
          const elif = getAttr(next, "t-elif");
          const isElse = getAttr(next, "t-else") !== void 0;
          if (elif === void 0 && !isElse) {
            break;
          }
          branches.push({
            cond: isElse ? null : elif,
            body: this.parseElement(next, ns, preserve, ["t-elif", "t-else"])
          });
          j = k + 1;
          if (isElse) {
            break;
          }
        }
        result.push({ type: "if", branches, pos: pos(node) });
        i = j;
        continue;
      }
      result.push(this.parseElement(node, ns, preserve, []));
      i++;
    }
    return result;
  }
  parseText(value, preserve, position) {
    var _a, _b;
    let text = value;
    if (!preserve) {
      if (!text.trim() && text.indexOf("\n") !== -1) {
        return null;
      }
      text = text.replace(/\s*\n\s*/g, " ");
    }
    const parts = splitInterpolation(text);
    if (parts === null) {
      return text ? { type: "text", value: text, raw: this.noTranslate || void 0 } : null;
    }
    const before = value.slice(0, value.indexOf("{{"));
    const line = (position == null ? void 0 : position.line) !== void 0 ? position.line + ((_b = (_a = before.match(/\n/g)) == null ? void 0 : _a.length) != null ? _b : 0) : void 0;
    return { type: "textExpr", parts, pos: { line, origin: position == null ? void 0 : position.origin } };
  }
  parseElement(el, ns, preserve, handled) {
    if (getAttr(el, "t-translation") === "off" && !this.noTranslate) {
      this.noTranslate = true;
      try {
        return this.parseElementInner(el, ns, preserve, handled);
      } finally {
        this.noTranslate = false;
      }
    }
    return this.parseElementInner(el, ns, preserve, handled);
  }
  parseElementInner(el, ns, preserve, handled) {
    var _a;
    const attr = (name) => handled.includes(name) ? void 0 : getAttr(el, name);
    for (const { name } of el.attrs) {
      if (name.startsWith("t-") && !KNOWN_DIRECTIVES.has(name) && !name.startsWith("t-att-") && !name.startsWith("t-on-")) {
        throw new Error(`[trame] Directive inconnue "${name}" sur <${el.tag}>`);
      }
    }
    const collection = attr("t-foreach");
    if (collection !== void 0) {
      const as = getAttr(el, "t-as");
      if (as === void 0 || !isIdentifier(as)) {
        throw new Error(`[trame] t-foreach="${collection}" : t-as doit \xEAtre un nom de variable valide`);
      }
      const key2 = (_a = getAttr(el, "t-key")) != null ? _a : null;
      const inner = this.parseElement(el, ns, preserve, [...handled, "t-foreach", "t-as", "t-key"]);
      let body = inner;
      const cond = getAttr(el, "t-if");
      if (cond !== void 0 && !handled.includes("t-if")) {
        body = {
          type: "if",
          branches: [{ cond, body: this.parseElement(el, ns, preserve, [...handled, "t-foreach", "t-as", "t-key", "t-if"]) }],
          pos: pos(el)
        };
      }
      return { type: "foreach", collection, as, key: key2, body, pos: pos(el) };
    }
    const key = attr("t-key");
    if (key !== void 0) {
      return { type: "keyed", key, body: this.parseElement(el, ns, preserve, [...handled, "t-key"]), pos: pos(el) };
    }
    const set = attr("t-set");
    if (set !== void 0) {
      if (!isIdentifier(set)) {
        throw new Error(`[trame] t-set="${set}" : nom de variable invalide`);
      }
      const value = getAttr(el, "t-value");
      if (value === void 0) {
        throw new Error(`[trame] t-set="${set}" : t-value est obligatoire`);
      }
      return { type: "set", name: set, value, pos: pos(el) };
    }
    const call = attr("t-call");
    if (call !== void 0) {
      return { type: "call", template: call, params: this.plainParams(el, ["t-call"]), pos: pos(el) };
    }
    const slot = attr("t-slot");
    if (slot !== void 0) {
      const fallbackChildren = this.parseChildren(el.children, ns, preserve);
      return {
        type: "slot",
        name: slot || "default",
        params: this.plainParams(el, ["t-slot"]),
        fallback: fallbackChildren.length ? toSingle(fallbackChildren) : null,
        pos: pos(el)
      };
    }
    const dynamicComponent = attr("t-component");
    const isComponent = dynamicComponent !== void 0 || /^[A-Z]/.test(el.tag) || BUILTIN_COMPONENTS.has(el.tag);
    if (isComponent) {
      return this.parseComponent(el, dynamicComponent != null ? dynamicComponent : null, ns, preserve);
    }
    const out = attr("t-out");
    if (el.tag === "t") {
      if (out !== void 0) {
        return { type: "out", expr: out, pos: pos(el) };
      }
      const children = this.parseChildren(el.children, ns, preserve);
      return toSingle(children);
    }
    const childNs = el.tag === "svg" ? "svg" : el.tag === "math" ? "math" : el.tag === "foreignObject" ? "html" : ns;
    const elementNs = el.tag === "svg" ? "svg" : el.tag === "math" ? "math" : ns;
    const node = {
      type: "element",
      tag: el.tag,
      ns: elementNs,
      attrs: [],
      dynAttrs: [],
      events: [],
      children: [],
      noTranslate: this.noTranslate || void 0,
      pos: pos(el)
    };
    for (const { name, value } of el.attrs) {
      if (name === "t-ref") {
        node.ref = value;
      } else if (name === "t-att") {
        node.attrsSpread = value;
      } else if (name.startsWith("t-att-")) {
        node.dynAttrs.push({ name: name.slice(6), expr: value });
      } else if (name.startsWith("t-on-")) {
        const [event, ...modifiers] = name.slice(5).split(".");
        if (!event) {
          throw new Error(`[trame] <${el.tag}> : nom d'\xE9v\xE9nement manquant dans "${name}"`);
        }
        node.events.push({ name: event, modifiers, expr: value });
      } else if (name.startsWith("t-")) {
        if (HANDLED_ELSEWHERE.has(name)) {
          continue;
        }
        throw new Error(`[trame] La directive "${name}" n'est pas utilisable sur <${el.tag}>`);
      } else {
        const parts = splitInterpolation(value);
        if (parts === null) {
          node.attrs.push([name, value]);
        } else {
          node.dynAttrs.push({ name, parts });
        }
      }
    }
    const keepSpaces = preserve || PRESERVE_WHITESPACE.has(el.tag);
    if (out !== void 0) {
      node.children = [{ type: "out", expr: out, pos: pos(el) }];
    } else {
      node.children = this.parseChildren(el.children, childNs, keepSpaces);
    }
    return node;
  }
  parseComponent(el, dynamic, ns, preserve) {
    var _a, _b;
    const node = {
      type: "component",
      name: dynamic === null ? el.tag : null,
      dynamic,
      props: [],
      spread: null,
      slots: [],
      pos: pos(el)
    };
    for (const { name, value } of el.attrs) {
      if (name === "t-props") {
        node.spread = value;
      } else if (name === "t-component" || name === "t-slot-scope" || HANDLED_ELSEWHERE.has(name)) {
        continue;
      } else if (name.startsWith("t-")) {
        throw new Error(`[trame] La directive "${name}" n'est pas utilisable sur le composant <${el.tag}>`);
      } else {
        const parts = splitInterpolation(value);
        if (parts !== null && !(parts.length === 1 && parts[0].expr !== void 0 && value.trim().startsWith("{{"))) {
          node.props.push({ name, parts });
        } else if (parts !== null) {
          node.props.push({ name, expr: parts[0].expr });
        } else {
          node.props.push({ name, expr: value });
        }
      }
    }
    const defaultChildren = [];
    for (const child of el.children) {
      if (child.type === "element" && getAttr(child, "t-set-slot") !== void 0) {
        const slotName = getAttr(child, "t-set-slot");
        const scope = (_a = getAttr(child, "t-slot-scope")) != null ? _a : null;
        if (scope !== null && !isIdentifier(scope)) {
          throw new Error(`[trame] t-slot-scope="${scope}" : nom de variable invalide`);
        }
        const inner = child.tag === "t" ? toSingle(this.parseChildren(child.children, ns, preserve)) : this.parseElement(child, ns, preserve, ["t-set-slot", "t-slot-scope"]);
        node.slots.push({ name: slotName, scope, body: inner });
      } else {
        defaultChildren.push(child);
      }
    }
    const defaultBody = this.parseChildren(defaultChildren, ns, preserve);
    if (defaultBody.length && !node.slots.some((s) => s.name === "default")) {
      const scope = (_b = getAttr(el, "t-slot-scope")) != null ? _b : null;
      node.slots.push({ name: "default", scope, body: toSingle(defaultBody) });
    }
    return node;
  }
  /** Attributs d'un t-call / t-slot : paramètres (expressions). */
  plainParams(el, skip) {
    const params = [];
    for (const { name, value } of el.attrs) {
      if (skip.includes(name) || name.startsWith("t-")) {
        continue;
      }
      if (!isIdentifier(name)) {
        throw new Error(`[trame] Param\xE8tre invalide "${name}" sur <${el.tag}>`);
      }
      params.push({ name, expr: value });
    }
    return params;
  }
};
function pos(node) {
  return { line: node.line, origin: node.origin };
}
function toSingle(children) {
  return children.length === 1 ? children[0] : { type: "multi", children };
}

// src/i18n.ts
var translator = null;
function setTranslator(fn) {
  translator = fn;
}
function _t(text) {
  return translator === null ? text : translator(text);
}
function translateTemplateText(text) {
  if (translator === null) {
    return text;
  }
  const match = /^(\s*)([\s\S]*?)(\s*)$/.exec(text);
  const content = match[2];
  if (!/[^\s\d.,:;!?()\-+*/%\u20AC$#@&|'"\u00AB\u00BB\u2026]/.test(content)) {
    return text;
  }
  return match[1] + translator(content) + match[3];
}
var TRANSLATABLE_ATTRIBUTES = /* @__PURE__ */ new Set(["title", "placeholder", "alt", "aria-label", "label"]);

// src/reactivity/core.ts
var CLEAN = 0;
var CHECK = 1;
var DIRTY = 2;
var defaultEquals = Object.is;
var currentObserver = null;
var globalVersion = 0;
function getCurrentObserver() {
  return currentObserver;
}
function untrack(fn) {
  const prev = currentObserver;
  currentObserver = null;
  try {
    return fn();
  } finally {
    currentObserver = prev;
  }
}
var Link = class {
  constructor(source, observer) {
    __publicField(this, "source", source);
    __publicField(this, "observer", observer);
    /** Version de la source vue à la dernière lecture ; -1 : pas encore relue pendant l'exécution en cours. */
    __publicField(this, "version");
    __publicField(this, "nextSource", null);
    /** Liste des sources en construction pendant l'exécution. */
    __publicField(this, "nextTracked", null);
    __publicField(this, "prevObserver", null);
    __publicField(this, "nextObserver", null);
    /** Lien courant précédent de la source (exécutions imbriquées), restauré en fin d'exécution. */
    __publicField(this, "rollback");
    /** Créé pendant l'exécution en cours : à abonner à la fin si le calcul est vivant. */
    __publicField(this, "fresh", true);
    this.version = source.version;
  }
};
var ReactiveNode = class {
  constructor() {
    /** Incrémentée à chaque changement de valeur. */
    __publicField(this, "version", 0);
    /** Observateurs vivants (liste doublement chaînée de liens). */
    __publicField(this, "observersHead", null);
    __publicField(this, "observersTail", null);
    /**
     * Pendant l'exécution d'un calcul qui lit ce nœud : son lien vers ce calcul. Permet de réutiliser
     * le lien de l'exécution précédente et d'ignorer les lectures en double, sans structure auxiliaire.
     */
    __publicField(this, "currentLink");
  }
  track() {
    const obs = currentObserver;
    if (obs !== null) {
      obs.addSource(this);
    }
  }
  /** Abonne un lien (calcul vivant) à ce nœud. */
  linkObserver(link) {
    link.prevObserver = this.observersTail;
    link.nextObserver = null;
    if (this.observersTail !== null) {
      this.observersTail.nextObserver = link;
    } else {
      this.observersHead = link;
    }
    this.observersTail = link;
  }
  /** Désabonne un lien ; prévient le nœud s'il n'a plus d'observateur. */
  unlinkObserver(link) {
    const { prevObserver, nextObserver } = link;
    if (prevObserver === null && this.observersHead !== link) {
      return;
    }
    if (prevObserver !== null) {
      prevObserver.nextObserver = nextObserver;
    } else {
      this.observersHead = nextObserver;
    }
    if (nextObserver !== null) {
      nextObserver.prevObserver = prevObserver;
    } else {
      this.observersTail = prevObserver;
    }
    link.prevObserver = null;
    link.nextObserver = null;
    if (this.observersHead === null) {
      this.onUnobserved();
    }
  }
  get observed() {
    return this.observersHead !== null;
  }
  /** Nombre d'observateurs vivants (diagnostic, tests). */
  get observerCount() {
    let count = 0;
    for (let link = this.observersHead; link !== null; link = link.nextObserver) {
      count++;
    }
    return count;
  }
  /** Marque tous les observateurs vivants. */
  markObservers(state2) {
    for (let link = this.observersHead; link !== null; ) {
      const next = link.nextObserver;
      link.observer.mark(state2);
      link = next;
    }
  }
  /** Appelé quand le dernier observateur vivant disparaît. */
  onUnobserved() {
  }
  /** Appelé quand une lecture veut s'assurer que la valeur est à jour. */
  updateIfNeeded() {
  }
  /** Prévient les observateurs que la valeur a changé. */
  notify() {
    this.version++;
    globalVersion++;
    if (this.observersHead !== null) {
      batchDepth++;
      try {
        this.markObservers(DIRTY);
      } finally {
        batchDepth--;
      }
      if (batchDepth === 0) {
        scheduleFlush();
      }
    }
  }
};
var Signal = class extends ReactiveNode {
  constructor(value, equals = defaultEquals) {
    super();
    __publicField(this, "value");
    __publicField(this, "equals");
    this.value = value;
    this.equals = equals === false ? () => false : equals;
  }
  get() {
    this.track();
    return this.value;
  }
  /** Lit la valeur sans créer de dépendance. */
  peek() {
    return this.value;
  }
  set(value) {
    if (!this.equals(this.value, value)) {
      this.value = value;
      this.notify();
    }
  }
  /** Force la notification des observateurs (valeur mutée en place). */
  trigger() {
    this.notify();
  }
};
var tracker = null;
var trackedHead = null;
var trackedTail = null;
var trackedPending = null;
var TRACKED = 1;
var LIVE = 2;
var DISPOSED = 4;
var QUEUED = 8;
var Computation = class extends ReactiveNode {
  constructor() {
    super(...arguments);
    __publicField(this, "state", DIRTY);
    /** Sources lues lors de la dernière exécution (liste de liens, dans l'ordre de lecture). */
    __publicField(this, "sourcesHead", null);
    /** Ressources en attente lues (directement ou via un computed) lors de la dernière exécution. */
    __publicField(this, "pending", null);
    __publicField(this, "flags", 0);
  }
  /** Exécuté au moins une fois (ses sources sont connues) ? */
  get tracked() {
    return (this.flags & TRACKED) !== 0;
  }
  /** Abonné à ses sources ? */
  get live() {
    return (this.flags & LIVE) !== 0;
  }
  set live(value) {
    this.flags = value ? this.flags | LIVE : this.flags & ~LIVE;
  }
  get disposed() {
    return (this.flags & DISPOSED) !== 0;
  }
  set disposed(value) {
    this.flags = value ? this.flags | DISPOSED : this.flags & ~DISPOSED;
  }
  addSource(source) {
    if (tracker !== this) {
      return;
    }
    let link = source.currentLink;
    if (link !== void 0 && link.observer === this) {
      if (link.version !== -1 || link.fresh) {
        return;
      }
      link.version = source.version;
    } else {
      link = new Link(source, this);
      link.rollback = source.currentLink;
      source.currentLink = link;
    }
    if (trackedTail !== null) {
      trackedTail.nextTracked = link;
    } else {
      trackedHead = link;
    }
    trackedTail = link;
    if (source instanceof Computed && source.pending !== null) {
      for (const p of source.pending) {
        this.addPending(p);
      }
    }
  }
  addPending(source) {
    if (tracker === this) {
      (trackedPending != null ? trackedPending : trackedPending = /* @__PURE__ */ new Set()).add(source);
    }
  }
  /** Pendant l'exécution : une ressource en attente a-t-elle été lue ? */
  hasPendingReads() {
    return tracker === this && trackedPending !== null && trackedPending.size > 0;
  }
  /** Parcourt les sources lues lors de la dernière exécution. */
  forEachSource(fn) {
    for (let link = this.sourcesHead; link !== null; link = link.nextSource) {
      fn(link.source);
    }
  }
  /** Hors exécution : faut-il réexécuter (une source a-t-elle vraiment changé) ? */
  needsUpdate() {
    if (this.state === CLEAN) {
      return false;
    }
    if (this.state === CHECK && !this.sourcesChanged()) {
      this.state = CLEAN;
      return false;
    }
    return true;
  }
  /** Exécute `fn` en collectant les dépendances, puis met à jour les abonnements. */
  runTracked(fn) {
    const prevObserver = currentObserver;
    if (tracker === this) {
      currentObserver = this;
      try {
        return fn();
      } finally {
        currentObserver = prevObserver;
      }
    }
    const prevTracker = tracker;
    const prevHead = trackedHead;
    const prevTail = trackedTail;
    const prevPending = trackedPending;
    for (let link = this.sourcesHead; link !== null; link = link.nextSource) {
      link.rollback = link.source.currentLink;
      link.source.currentLink = link;
      link.version = -1;
    }
    tracker = this;
    trackedHead = null;
    trackedTail = null;
    trackedPending = null;
    currentObserver = this;
    try {
      return fn();
    } finally {
      currentObserver = prevObserver;
      const head = trackedHead;
      this.pending = trackedPending;
      tracker = prevTracker;
      trackedHead = prevHead;
      trackedTail = prevTail;
      trackedPending = prevPending;
      this.commitSources(head);
    }
  }
  /** Fin d'exécution : abandonne les sources non relues, abonne les nouvelles. */
  commitSources(newHead) {
    const live = this.live;
    for (let link = this.sourcesHead; link !== null; link = link.nextSource) {
      link.source.currentLink = link.rollback;
      link.rollback = void 0;
      if (link.version === -1 && live) {
        link.source.unlinkObserver(link);
      }
    }
    for (let link = newHead; link !== null; ) {
      const next = link.nextTracked;
      if (link.fresh) {
        link.source.currentLink = link.rollback;
        link.rollback = void 0;
        link.fresh = false;
        if (live) {
          subscribe(link);
        }
      }
      link.nextSource = next;
      link.nextTracked = null;
      link = next;
    }
    this.sourcesHead = newHead;
    this.flags |= TRACKED;
  }
  /** Abonne toutes les sources (passage à l'état vivant). */
  subscribeAll() {
    for (let link = this.sourcesHead; link !== null; link = link.nextSource) {
      subscribe(link);
    }
  }
  /** Vérifie si une source a changé depuis la dernière exécution. */
  sourcesChanged() {
    if (!this.tracked) {
      return true;
    }
    for (let link = this.sourcesHead; link !== null; link = link.nextSource) {
      link.source.updateIfNeeded();
      if (link.source.version !== link.version) {
        return true;
      }
    }
    return false;
  }
  unsubscribeAll() {
    if (this.live) {
      this.live = false;
      for (let link = this.sourcesHead; link !== null; link = link.nextSource) {
        link.source.unlinkObserver(link);
      }
    }
  }
};
function subscribe(link) {
  const source = link.source;
  source.linkObserver(link);
  if (source instanceof Computed && !source.live) {
    source.goLive();
  }
}
var recomputeListener = null;
function setRecomputeListener(listener) {
  const prev = recomputeListener;
  recomputeListener = listener;
  return prev;
}
var Computed = class extends Computation {
  constructor(fn, options = {}) {
    var _a;
    super();
    __publicField(this, "fn", fn);
    __publicField(this, "value");
    __publicField(this, "error");
    __publicField(this, "hasError", false);
    __publicField(this, "equals");
    __publicField(this, "lastGlobalVersion", -1);
    /** Recalcul forcé à la prochaine lecture (cache calculé en mode observation). */
    __publicField(this, "forced", false);
    this.equals = (_a = options.equals) != null ? _a : defaultEquals;
  }
  get() {
    if (this.disposed) {
      return this.value;
    }
    this.updateIfNeeded();
    this.track();
    if (this.hasError) {
      throw this.error;
    }
    return this.value;
  }
  peek() {
    return untrack(() => this.get());
  }
  /** Force un recalcul à la prochaine lecture. */
  invalidate() {
    this.forced = true;
  }
  updateIfNeeded() {
    if (this.forced) {
      this.recompute();
      return;
    }
    if (this.live) {
      if (this.state === CLEAN) {
        return;
      }
      if (this.state === CHECK && !this.sourcesChanged()) {
        this.state = CLEAN;
        return;
      }
    } else {
      if (this.lastGlobalVersion === globalVersion) {
        return;
      }
      if (this.tracked && !this.sourcesChanged()) {
        this.lastGlobalVersion = globalVersion;
        return;
      }
    }
    this.recompute();
  }
  recompute() {
    this.forced = false;
    recomputeListener == null ? void 0 : recomputeListener(this);
    let value;
    let error2;
    let failed = false;
    try {
      value = this.runTracked(this.fn);
    } catch (e) {
      error2 = e;
      failed = true;
    }
    this.state = CLEAN;
    this.lastGlobalVersion = globalVersion;
    if (failed) {
      if (this.pending !== null) {
        failed = false;
        value = void 0;
      } else {
        const changed2 = !this.hasError || this.error !== error2;
        this.hasError = true;
        this.error = error2;
        if (changed2) {
          this.version++;
        }
        return;
      }
    }
    const changed = this.hasError || this.version === 0 || !this.equals(this.value, value);
    this.hasError = false;
    this.error = void 0;
    if (changed) {
      this.value = value;
      this.version++;
    }
  }
  mark(state2) {
    if (this.state < state2) {
      const wasClean = this.state === CLEAN;
      this.state = state2;
      if (wasClean) {
        this.markObservers(CHECK);
      }
    }
  }
  goLive() {
    if (this.tracked && !this.forced) {
      this.updateIfNeeded();
    }
    this.live = true;
    this.state = this.tracked ? CLEAN : DIRTY;
    this.subscribeAll();
  }
  onUnobserved() {
    this.unsubscribeAll();
  }
  dispose() {
    this.unsubscribeAll();
    this.disposed = true;
  }
};
var PRIORITY_RESOURCE = 0;
var PRIORITY_RENDER = 1;
var PRIORITY_USER = 2;
var effectIds = 0;
var effectLocations = /* @__PURE__ */ new WeakMap();
var Effect = class extends Computation {
  constructor(fn, owner, priority = PRIORITY_USER) {
    super();
    __publicField(this, "fn", fn);
    __publicField(this, "owner", owner);
    /** Clé de tri dans la file : priorité, puis profondeur (parents d'abord), puis ordre de création. */
    __publicField(this, "sortKey");
    __publicField(this, "cleanup");
    this.flags = LIVE;
    this.sortKey = (priority * 1024 + Math.min(owner ? owner.depth : 0, 1023)) * 4294967296 + effectIds++;
    owner == null ? void 0 : owner.registerEffect(this);
  }
  /** Les ressources en attente lues font-elles attendre l'affichage (frontière du scope) ? */
  get waitsForPending() {
    return true;
  }
  get queued() {
    return (this.flags & QUEUED) !== 0;
  }
  set queued(value) {
    this.flags = value ? this.flags | QUEUED : this.flags & ~QUEUED;
  }
  /** Localisation dans un template (conservée en mode dev seulement). */
  get loc() {
    return effectLocations.get(this);
  }
  set loc(value) {
    var _a, _b;
    if (value !== void 0 && ((_b = (_a = this.owner) == null ? void 0 : _a.app) == null ? void 0 : _b.dev)) {
      effectLocations.set(this, value);
    }
  }
  /** Exécute l'effet immédiatement. */
  run() {
    if (this.disposed) {
      return;
    }
    this.state = CLEAN;
    this.runCleanup();
    try {
      this.cleanup = this.runTracked(this.fn);
    } catch (e) {
      if (this.pending === null) {
        this.handleError(e);
      }
    }
    const pending = this.pending;
    if (pending !== null && this.owner !== null && this.waitsForPending) {
      this.owner.waitFor(pending);
    }
  }
  handleError(e) {
    if (this.owner !== null) {
      this.owner.handleError(annotateError(e, this.loc, this.owner));
    } else {
      reportError(e);
    }
  }
  runCleanup() {
    const cleanup = this.cleanup;
    if (typeof cleanup === "function") {
      this.cleanup = void 0;
      untrack(cleanup);
    }
  }
  mark(state2) {
    if (this.state < state2) {
      this.state = state2;
      this.schedule();
    }
  }
  schedule() {
    if (!this.queued && !this.disposed) {
      this.queued = true;
      queue.push(this);
      scheduleFlush();
    }
  }
  /** Appelé par le flush : vérifie les sources puis exécute si nécessaire. */
  update() {
    if (this.disposed || this.state === CLEAN) {
      return;
    }
    if (this.state === CHECK && !this.sourcesChanged()) {
      this.state = CLEAN;
      return;
    }
    this.run();
  }
  dispose() {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.unsubscribeAll();
    this.runCleanup();
  }
};
var EffectQueue = class {
  constructor() {
    __publicField(this, "heap", []);
  }
  get length() {
    return this.heap.length;
  }
  push(effect2) {
    const heap = this.heap;
    const key = effect2.sortKey;
    let i = heap.length;
    heap.push(effect2);
    while (i > 0) {
      const parent = i - 1 >> 1;
      if (heap[parent].sortKey <= key) {
        break;
      }
      heap[i] = heap[parent];
      i = parent;
    }
    heap[i] = effect2;
  }
  pop() {
    const heap = this.heap;
    const top = heap[0];
    const last = heap.pop();
    const n = heap.length;
    if (n > 0) {
      const key = last.sortKey;
      let i = 0;
      for (; ; ) {
        const left = 2 * i + 1;
        if (left >= n) {
          break;
        }
        const right = left + 1;
        const child = right < n && heap[right].sortKey < heap[left].sortKey ? right : left;
        if (heap[child].sortKey >= key) {
          break;
        }
        heap[i] = heap[child];
        i = child;
      }
      heap[i] = last;
    }
    return top;
  }
  clear() {
    this.heap = [];
  }
};
var queue = new EffectQueue();
var batchDepth = 0;
var flushScheduled = false;
var flushing = false;
var afterFlushCallbacks = [];
var scheduleMicrotask = typeof queueMicrotask === "function" ? queueMicrotask : (fn) => void Promise.resolve().then(fn);
function scheduleFlush() {
  if (!flushScheduled && batchDepth === 0 && !flushing) {
    flushScheduled = true;
    scheduleMicrotask(flush);
  }
}
function flush() {
  flushScheduled = false;
  if (flushing) {
    return;
  }
  flushing = true;
  let iterations = 0;
  try {
    while (queue.length > 0 || afterFlushCallbacks.length > 0) {
      while (queue.length > 0) {
        if (++iterations > 1e6) {
          queue.clear();
          reportError(new Error("[trame] Boucle r\xE9active infinie d\xE9tect\xE9e (un effet modifie ses propres d\xE9pendances ?)"));
          break;
        }
        const effect2 = queue.pop();
        effect2.queued = false;
        effect2.update();
      }
      if (afterFlushCallbacks.length > 0) {
        const callbacks = afterFlushCallbacks.splice(0);
        for (const cb of callbacks) {
          try {
            cb();
          } catch (e) {
            reportError(e);
          }
        }
      }
    }
  } finally {
    flushing = false;
  }
}
function batch(fn) {
  batchDepth++;
  try {
    return fn();
  } finally {
    batchDepth--;
    if (batchDepth === 0 && !flushing && (queue.length > 0 || afterFlushCallbacks.length > 0)) {
      flush();
    }
  }
}
function groupWrites(fn) {
  batchDepth++;
  try {
    return fn();
  } finally {
    batchDepth--;
    if (batchDepth === 0 && (queue.length > 0 || afterFlushCallbacks.length > 0)) {
      scheduleFlush();
    }
  }
}
function afterFlush(fn) {
  afterFlushCallbacks.push(fn);
  if (!flushing && batchDepth === 0 && !flushScheduled) {
    flushScheduled = true;
    scheduleMicrotask(flush);
  }
}
function nextTick() {
  return new Promise((resolve) => afterFlush(resolve));
}
function reportError(e) {
  console.error(e);
}
function annotateError(error2, loc, owner) {
  var _a;
  if (loc === void 0 || !(error2 instanceof Error) || !((_a = owner == null ? void 0 : owner.app) == null ? void 0 : _a.dev)) {
    return error2;
  }
  const e = error2;
  if (e.trameLocation !== void 0) {
    return error2;
  }
  try {
    Object.defineProperty(e, "trameLocation", { value: loc, enumerable: false, configurable: true });
    const header = `${e.name}: ${e.message}`;
    const line = `
    \u2192 ${loc}`;
    if (typeof e.stack === "string" && e.stack.startsWith(header)) {
      e.stack = header + line + e.stack.slice(header.length);
    } else {
      e.stack = header + line + (typeof e.stack === "string" ? "\n" + e.stack : "");
    }
  } catch {
  }
  return error2;
}

// src/reactivity/resource.ts
function isAbortError(e) {
  return e !== null && typeof e === "object" && e.name === "AbortError";
}
var ResourceSignal = class extends Signal {
  constructor(resource2) {
    super(void 0, false);
    __publicField(this, "resource", resource2);
  }
};
var Transition = class {
  constructor() {
    __publicField(this, "members", /* @__PURE__ */ new Map());
    __publicField(this, "closed", false);
  }
  add(resource2) {
    this.members.set(resource2, { done: false, value: void 0 });
  }
  resolve(resource2, value) {
    const member = this.members.get(resource2);
    if (member !== void 0) {
      member.done = true;
      member.value = value;
      this.check();
    }
  }
  drop(resource2) {
    if (this.members.delete(resource2)) {
      this.check();
    }
  }
  close() {
    this.closed = true;
    this.check();
  }
  check() {
    if (!this.closed || this.members.size === 0) {
      return;
    }
    for (const member of this.members.values()) {
      if (!member.done) {
        return;
      }
    }
    const entries = Array.from(this.members);
    this.members.clear();
    batch(() => {
      for (const [resource2, member] of entries) {
        resource2.commitFromTransition(member.value);
      }
    });
  }
};
var openTransition = null;
function currentTransition() {
  if (openTransition === null) {
    const transition = new Transition();
    openTransition = transition;
    scheduleMicrotask(() => {
      if (openTransition === transition) {
        openTransition = null;
      }
      transition.close();
    });
  }
  return openTransition;
}
var ResourceTracker = class extends Effect {
  constructor(resource2, owner) {
    super(() => resource2.execute(), owner, PRIORITY_RESOURCE);
    __publicField(this, "resource", resource2);
  }
  mark(state2) {
    if (this.state < state2) {
      this.state = state2;
      this.resource.onDependenciesChanged();
    }
  }
};
var Resource = class {
  constructor(fetcher, owner, options = {}, source = null) {
    __publicField(this, "fetcher", fetcher);
    __publicField(this, "owner", owner);
    __publicField(this, "options", options);
    __publicField(this, "source", source);
    __publicField(this, "waiters", /* @__PURE__ */ new Set());
    __publicField(this, "valueSig", new ResourceSignal(this));
    __publicField(this, "loadingSig", new Signal(false));
    __publicField(this, "errorSig", new Signal(void 0));
    __publicField(this, "hasValue", false);
    __publicField(this, "stale", true);
    __publicField(this, "runId", 0);
    __publicField(this, "controller", null);
    __publicField(this, "unlinkOwner", null);
    __publicField(this, "tracker", null);
    __publicField(this, "transition", null);
    __publicField(this, "disposed", false);
    owner == null ? void 0 : owner.onCleanup(() => this.dispose());
    if (options.eager) {
      this.start();
    }
  }
  /** Lecture de la valeur : déclenche le chargement si nécessaire. */
  read() {
    const observer = getCurrentObserver();
    if (observing > 0) {
      return this.valueSig.get();
    }
    if (this.stale && !this.disposed && (this.tracker === null || this.tracker.needsUpdate() || !this.hasValue)) {
      this.start();
    }
    this.stale = false;
    const value = this.valueSig.get();
    if (!this.hasValue && observer !== null && this.loadingSig.peek()) {
      observer.addPending(this);
    }
    return value;
  }
  /** Écriture locale (mise à jour optimiste) : aucune requête n'est lancée. */
  write(value) {
    const firstValue = !this.hasValue;
    groupWrites(() => {
      this.hasValue = true;
      this.valueSig.set(value);
    });
    if (firstValue) {
      this.notifyWaiters();
    }
  }
  isLoading() {
    return this.loadingSig.get();
  }
  getError() {
    return this.errorSig.get();
  }
  /** Relance la requête. */
  refresh() {
    if (!this.disposed) {
      this.start();
    }
  }
  get observed() {
    return this.valueSig.observed || this.loadingSig.observed || this.errorSig.observed;
  }
  onDependenciesChanged() {
    if (this.disposed) {
      return;
    }
    if (this.observed || this.options.eager) {
      this.tracker.schedule();
    } else {
      this.stale = true;
    }
  }
  start() {
    var _a;
    this.stale = false;
    (_a = this.tracker) != null ? _a : this.tracker = new ResourceTracker(this, this.owner);
    this.tracker.run();
  }
  /** Exécuté par le tracker, dans un contexte qui suit les dépendances. */
  execute() {
    var _a;
    if (this.disposed) {
      return;
    }
    const id = ++this.runId;
    this.abortCurrent();
    (_a = this.transition) == null ? void 0 : _a.drop(this);
    this.transition = null;
    const controller = new AbortController();
    this.controller = controller;
    const owner = this.owner;
    if (owner !== null) {
      const ownerSignal = owner.abortSignal;
      if (ownerSignal.aborted) {
        controller.abort();
      } else {
        const onAbort = () => controller.abort();
        ownerSignal.addEventListener("abort", onAbort);
        this.unlinkOwner = () => ownerSignal.removeEventListener("abort", onAbort);
      }
    }
    const tracker2 = this.tracker;
    const wasLoaded = this.hasValue;
    this.loadingSig.set(true);
    this.errorSig.set(void 0);
    let result;
    const ctx = { signal: controller.signal };
    try {
      if (this.source !== null) {
        const value = this.source();
        if (tracker2.hasPendingReads()) {
          return;
        }
        const fetcher = this.fetcher;
        result = untrack(() => fetcher(value, ctx));
      } else {
        result = this.fetcher(ctx);
      }
    } catch (e) {
      if (tracker2.hasPendingReads()) {
        throw e;
      }
      this.fail(id, e);
      return;
    }
    if (result !== null && typeof result === "object" && typeof result.then === "function") {
      const waitingOnDependency = tracker2.hasPendingReads();
      if (wasLoaded && !waitingOnDependency) {
        const transition = currentTransition();
        transition.add(this);
        this.transition = transition;
      }
      if (waitingOnDependency) {
        result.then(void 0, () => {
        });
        return;
      }
      result.then(
        (value) => this.settle(id, value),
        (error2) => this.fail(id, error2)
      );
    } else {
      this.settle(id, result);
    }
  }
  settle(id, value) {
    if (id !== this.runId || this.disposed) {
      return;
    }
    if (this.transition !== null) {
      this.transition.resolve(this, value);
    } else {
      this.commit(value);
    }
  }
  commitFromTransition(value) {
    this.transition = null;
    this.commit(value);
  }
  commit(value) {
    this.releaseController();
    const firstValue = !this.hasValue;
    batch(() => {
      this.hasValue = true;
      this.valueSig.set(value);
      this.loadingSig.set(false);
    });
    if (firstValue) {
      this.notifyWaiters();
    }
  }
  fail(id, error2) {
    var _a;
    if (id !== this.runId || this.disposed) {
      return;
    }
    this.releaseController();
    (_a = this.transition) == null ? void 0 : _a.drop(this);
    this.transition = null;
    if (isAbortError(error2) && this.waiters.size === 0) {
      this.loadingSig.set(false);
      return;
    }
    batch(() => {
      this.errorSig.set(error2);
      this.loadingSig.set(false);
    });
    if (this.waiters.size > 0) {
      const waiters = Array.from(this.waiters);
      this.waiters.clear();
      for (const waiter of waiters) {
        waiter.failed(this, error2);
      }
    } else if (!this.errorSig.observed) {
      reportError(error2);
    }
  }
  notifyWaiters() {
    if (this.waiters.size > 0) {
      const waiters = Array.from(this.waiters);
      this.waiters.clear();
      for (const waiter of waiters) {
        waiter.resolved(this);
      }
    }
  }
  releaseController() {
    var _a;
    (_a = this.unlinkOwner) == null ? void 0 : _a.call(this);
    this.unlinkOwner = null;
    this.controller = null;
  }
  abortCurrent() {
    const controller = this.controller;
    this.releaseController();
    controller == null ? void 0 : controller.abort();
  }
  dispose() {
    var _a, _b;
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.runId++;
    this.abortCurrent();
    (_a = this.transition) == null ? void 0 : _a.drop(this);
    this.transition = null;
    (_b = this.tracker) == null ? void 0 : _b.dispose();
    this.waiters.clear();
  }
};
var PeekComputation = class extends Computation {
  mark() {
  }
};
var observing = 0;
function collectResources(fn) {
  if (typeof fn !== "function") {
    throw new TypeError(
      "[trame] loading()/error()/refresh() attendent une fonction en TypeScript : loading(() => this.order). Dans les templates, \xE9crivez simplement loading(order)."
    );
  }
  const peek = new PeekComputation();
  const recomputed = [];
  const prevListener = setRecomputeListener((c) => recomputed.push(c));
  observing++;
  try {
    peek.runTracked(fn);
  } catch {
  } finally {
    observing--;
    setRecomputeListener(prevListener);
  }
  const outer = getCurrentObserver();
  if (outer !== null) {
    peek.forEachSource((source) => outer.addSource(source));
  }
  const found = [];
  const seen = /* @__PURE__ */ new Set();
  const walk = (node) => {
    if (seen.has(node)) {
      return;
    }
    seen.add(node);
    if (node instanceof ResourceSignal) {
      found.push(node.resource);
    } else if (node instanceof Computed) {
      node.forEachSource(walk);
    }
  };
  peek.forEachSource(walk);
  for (const c of recomputed) {
    c.invalidate();
  }
  return found;
}
function loading(fn) {
  let result = false;
  for (const resource2 of collectResources(fn)) {
    if (resource2.isLoading()) {
      result = true;
    }
  }
  return result;
}
function error(fn) {
  let result = void 0;
  for (const resource2 of collectResources(fn)) {
    const e = resource2.getError();
    if (result === void 0 && e !== void 0) {
      result = e;
    }
  }
  return result;
}
function refresh(fn) {
  var _a;
  const resources = untrack(() => collectResources(fn));
  (_a = resources[resources.length - 1]) == null ? void 0 : _a.refresh();
}

// src/reactivity/owner.ts
var currentOwner = null;
function getOwner() {
  return currentOwner;
}
function runWithOwner(owner, fn) {
  const prev = currentOwner;
  currentOwner = owner;
  try {
    return fn();
  } finally {
    currentOwner = prev;
  }
}
var AmbiguousService = class {
  constructor(candidates) {
    __publicField(this, "candidates", candidates);
  }
};
var Owner = class {
  constructor(parent = currentOwner) {
    __publicField(this, "parent");
    __publicField(this, "depth");
    __publicField(this, "app");
    __publicField(this, "boundary");
    /** Le scope est-il affiché dans un DOM vivant ? */
    __publicField(this, "live", false);
    /** Contenu préparé mais pas encore inséré (en attente de données). */
    __publicField(this, "detached", false);
    __publicField(this, "disposed", false);
    __publicField(this, "children", null);
    __publicField(this, "effects", null);
    __publicField(this, "cleanups", null);
    __publicField(this, "extra", null);
    var _a;
    this.parent = parent;
    this.depth = parent ? parent.depth + 1 : 0;
    this.app = parent ? parent.app : null;
    this.boundary = parent ? parent.boundary : null;
    if (parent) {
      if (parent.disposed) {
        this.disposed = true;
      } else {
        ((_a = parent.children) != null ? _a : parent.children = /* @__PURE__ */ new Set()).add(this);
      }
    }
  }
  get x() {
    var _a;
    return (_a = this.extra) != null ? _a : this.extra = { errorHandler: null, actionHandler: null, providers: null, inherited: null, mountCallbacks: null, controller: null };
  }
  /** Gestionnaire d'erreurs local : renvoie true si l'erreur est prise en charge. */
  get errorHandler() {
    return this.extra === null ? null : this.extra.errorHandler;
  }
  set errorHandler(handler) {
    this.x.errorHandler = handler;
  }
  /** Gestionnaire des erreurs d'actions (<ErrorHandler>) : renvoie true si l'erreur est prise en charge. */
  get actionHandler() {
    return this.extra === null ? null : this.extra.actionHandler;
  }
  set actionHandler(handler) {
    this.x.actionHandler = handler;
  }
  /** AbortSignal déclenché à la destruction du scope. */
  get abortSignal() {
    var _a, _b;
    const controller = (_b = (_a = this.x).controller) != null ? _b : _a.controller = new AbortController();
    if (this.disposed && !controller.signal.aborted) {
      controller.abort();
    }
    return controller.signal;
  }
  registerEffect(effect2) {
    var _a;
    if (this.disposed) {
      effect2.dispose();
      return;
    }
    ((_a = this.effects) != null ? _a : this.effects = []).push(effect2);
  }
  onCleanup(fn) {
    var _a;
    if (this.disposed) {
      untrack(fn);
      return;
    }
    ((_a = this.cleanups) != null ? _a : this.cleanups = []).push(fn);
  }
  /** Exécute `fn` quand le scope sera affiché (immédiatement s'il l'est déjà). */
  onMount(fn) {
    var _a, _b;
    if (this.disposed) {
      return;
    }
    if (this.live) {
      fn();
    } else {
      ((_b = (_a = this.x).mountCallbacks) != null ? _b : _a.mountCallbacks = []).push(fn);
    }
  }
  /** Marque le scope (et ses enfants insérés) comme affiché, et déclenche les callbacks onMount. */
  activate() {
    if (this.live || this.disposed || this.detached) {
      return;
    }
    if (this.children !== null) {
      for (const child of this.children) {
        child.activate();
      }
    }
    this.live = true;
    const callbacks = this.extra === null ? null : this.extra.mountCallbacks;
    if (callbacks !== null) {
      this.extra.mountCallbacks = null;
      for (const cb of callbacks) {
        try {
          cb();
        } catch (e) {
          this.handleError(e);
        }
      }
    }
  }
  /**
   * Cherche un service fourni par ce scope ou un ancêtre (le plus proche l'emporte). À un même
   * niveau, un service fourni sous sa classe exacte l'emporte sur un service dont c'est une classe parente.
   */
  lookup(key) {
    let owner = this;
    while (owner !== null) {
      const extra = owner.extra;
      if (extra !== null) {
        if (extra.providers !== null && extra.providers.has(key)) {
          return extra.providers.get(key);
        }
        if (extra.inherited !== null && extra.inherited.has(key)) {
          return extra.inherited.get(key);
        }
      }
      owner = owner.parent;
    }
    return void 0;
  }
  /** Fournit `value` sous `key` (classe exacte ou clé explicite) : une seule fois par scope. */
  provide(key, value, describe2 = String) {
    var _a, _b;
    const providers = (_b = (_a = this.x).providers) != null ? _b : _a.providers = /* @__PURE__ */ new Map();
    if (providers.has(key)) {
      throw new Error(
        `[trame] Service ${describe2(key)} fourni deux fois au m\xEAme niveau (${describe2(providers.get(key))} puis ${describe2(value)}). Un enfant peut le red\xE9finir pour ses descendants ; pour modifier le service, utilisez patch().`
      );
    }
    providers.set(key, value);
  }
  /** Fournit `value` sous une de ses classes parentes. Deux services différents : injection ambiguë. */
  provideInherited(key, value) {
    var _a, _b;
    const inherited = (_b = (_a = this.x).inherited) != null ? _b : _a.inherited = /* @__PURE__ */ new Map();
    const existing = inherited.get(key);
    if (existing === void 0) {
      inherited.set(key, value);
    } else if (existing instanceof AmbiguousService) {
      existing.candidates.push(value);
    } else if (existing !== value) {
      inherited.set(key, new AmbiguousService([existing, value]));
    }
  }
  /** Remplace une valeur fournie (service instancié à la première demande) sous toutes ses clés. */
  replaceProvided(from, to) {
    if (this.extra === null) {
      return;
    }
    for (const map of [this.extra.providers, this.extra.inherited]) {
      if (map === null) {
        continue;
      }
      for (const [k, v] of map) {
        if (v === from) {
          map.set(k, to);
        } else if (v instanceof AmbiguousService) {
          v.candidates = v.candidates.map((c) => c === from ? to : c);
        }
      }
    }
  }
  /** Signale à la frontière d'attente les ressources lues pendant leur premier chargement. */
  waitFor(pending) {
    const boundary = this.boundary;
    if (boundary !== null) {
      for (const source of pending) {
        boundary.wait(source, this);
      }
    }
  }
  /**
   * Erreur d'une action (gestionnaire d'événement, y compris une promesse rejetée) : elle remonte au
   * <ErrorHandler> le plus proche, sans passer par les <ErrorBoundary> (le contenu reste affiché),
   * puis à l'application (onError, sinon la console ; l'application reste montée).
   */
  handleActionError(error2) {
    let owner = this;
    while (owner !== null) {
      const handler = owner.extra === null ? null : owner.extra.actionHandler;
      if (handler !== null && !owner.disposed) {
        try {
          if (handler(error2)) {
            return;
          }
        } catch (e) {
          error2 = e;
        }
      }
      owner = owner.parent;
    }
    if (this.app !== null) {
      this.app.handleActionError(error2);
    } else {
      reportError(error2);
    }
  }
  handleError(error2) {
    let owner = this;
    while (owner !== null) {
      const handler = owner.errorHandler;
      if (handler !== null && !owner.disposed) {
        try {
          if (handler(error2)) {
            return;
          }
        } catch (e) {
          error2 = e;
        }
      }
      owner = owner.parent;
    }
    if (this.app !== null) {
      this.app.handleUncaughtError(error2);
    } else {
      reportError(error2);
    }
  }
  dispose() {
    var _a, _b, _c;
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.live = false;
    if (this.children !== null) {
      const children = Array.from(this.children);
      this.children = null;
      for (let i = children.length - 1; i >= 0; i--) {
        children[i].dispose();
      }
    }
    if (this.effects !== null) {
      const effects = this.effects;
      this.effects = null;
      for (let i = effects.length - 1; i >= 0; i--) {
        effects[i].dispose();
      }
    }
    if (this.cleanups !== null) {
      const cleanups = this.cleanups;
      this.cleanups = null;
      for (let i = cleanups.length - 1; i >= 0; i--) {
        try {
          untrack(cleanups[i]);
        } catch (e) {
          reportError(e);
        }
      }
    }
    if (this.extra !== null) {
      this.extra.mountCallbacks = null;
      (_a = this.extra.controller) == null ? void 0 : _a.abort();
    }
    (_c = (_b = this.parent) == null ? void 0 : _b.children) == null ? void 0 : _c.delete(this);
  }
};
var Boundary = class {
  constructor(onReady, onError) {
    __publicField(this, "onReady", onReady);
    __publicField(this, "onError", onError);
    __publicField(this, "pending", /* @__PURE__ */ new Set());
    /** Premier scope ayant lu chaque ressource attendue : il reçoit l'erreur si elle échoue. */
    __publicField(this, "readers", /* @__PURE__ */ new Map());
    __publicField(this, "building", true);
    __publicField(this, "settled", false);
    __publicField(this, "closed", false);
  }
  get isPending() {
    return !this.settled;
  }
  /** Des ressources en premier chargement ont-elles été lues (et sont-elles attendues) ? */
  get waiting() {
    return this.pending.size > 0;
  }
  wait(source, reader) {
    if (this.settled || this.closed || this.pending.has(source)) {
      return;
    }
    this.pending.add(source);
    if (reader !== void 0) {
      this.readers.set(source, reader);
    }
    source.waiters.add(this);
  }
  resolved(source) {
    this.readers.delete(source);
    if (this.pending.delete(source)) {
      this.scheduleCheck();
    }
  }
  /**
   * Une ressource attendue a échoué : l'erreur est transmise au scope qui l'a lue (une
   * <ErrorBoundary> englobante peut ainsi l'intercepter), puis l'attente continue pour le reste.
   */
  failed(source, error2) {
    if (!this.pending.has(source) || this.closed) {
      return;
    }
    this.pending.delete(source);
    const reader = this.readers.get(source);
    this.readers.delete(source);
    if (reader === void 0 || reader.disposed) {
      this.cancel();
      this.onError(error2);
      return;
    }
    reader.handleError(error2);
    this.scheduleCheck();
  }
  scheduleCheck() {
    if (this.pending.size === 0 && !this.building && !this.closed) {
      afterFlush(() => this.check());
    }
  }
  /** La construction synchrone est terminée : on peut devenir prêt dès que plus rien n'est attendu. */
  done() {
    this.building = false;
    this.check();
  }
  check() {
    if (!this.settled && !this.closed && !this.building && this.pending.size === 0) {
      this.settled = true;
      this.onReady();
    }
  }
  /** Abandonne l'attente (contenu détruit avant d'être prêt). */
  cancel() {
    this.closed = true;
    for (const source of this.pending) {
      source.waiters.delete(this);
    }
    this.pending.clear();
    this.readers.clear();
  }
};

// src/patch.ts
var classPatches = /* @__PURE__ */ new Map();
var initialized = /* @__PURE__ */ new WeakMap();
var patchVersion = 0;
var upToDate = /* @__PURE__ */ new WeakMap();
var stampTarget = null;
var Stamp = class {
  constructor() {
    return stampTarget;
  }
};
function runPatchFields(instance, patch2) {
  const saved = Object.getPrototypeOf(patch2.patchClass);
  const prevTarget = stampTarget;
  Object.setPrototypeOf(patch2.patchClass, Stamp);
  stampTarget = instance;
  try {
    Reflect.construct(patch2.patchClass, []);
  } finally {
    stampTarget = prevTarget;
    Object.setPrototypeOf(patch2.patchClass, saved);
  }
}
function initPatches(instance) {
  if (classPatches.size === 0 || upToDate.get(instance) === patchVersion) {
    return;
  }
  const obj = instance;
  const chain = [];
  for (let proto = Object.getPrototypeOf(obj); proto !== null && proto !== Object.prototype; proto = Object.getPrototypeOf(proto)) {
    if (Object.prototype.hasOwnProperty.call(proto, "constructor")) {
      chain.unshift(proto.constructor);
    }
  }
  let done = initialized.get(obj);
  for (const cls of chain) {
    const patches = classPatches.get(cls);
    if (patches === void 0) {
      continue;
    }
    for (const p of patches) {
      if (done == null ? void 0 : done.has(p)) {
        continue;
      }
      if (done === void 0) {
        done = /* @__PURE__ */ new Set();
        initialized.set(obj, done);
      }
      done.add(p);
      runPatchFields(obj, p);
    }
  }
  upToDate.set(obj, patchVersion);
}
function findDescriptor(proto, key) {
  for (let p = proto; p !== null; p = Object.getPrototypeOf(p)) {
    const descriptor = Object.getOwnPropertyDescriptor(p, key);
    if (descriptor !== void 0) {
      return descriptor;
    }
  }
  return void 0;
}
function withLazyInit(descriptor) {
  const wrap = (fn) => {
    const wrapper = function(...args) {
      if (this !== null && typeof this === "object") {
        initPatches(this);
      }
      return fn.apply(this, args);
    };
    for (const symbol of Object.getOwnPropertySymbols(fn)) {
      wrapper[symbol] = fn[symbol];
    }
    return wrapper;
  };
  const result = { ...descriptor };
  if (typeof descriptor.value === "function") {
    result.value = wrap(descriptor.value);
  }
  if (descriptor.get) {
    result.get = wrap(descriptor.get);
  }
  if (descriptor.set) {
    result.set = wrap(descriptor.set);
  }
  return result;
}
var STATIC_SKIP = /* @__PURE__ */ new Set(["length", "name", "prototype", "caller", "arguments"]);
function isClass(value) {
  return typeof value === "function" && value.prototype !== void 0 && /^class[\s{]/.test(Function.prototype.toString.call(value));
}
function patch(target, extension) {
  var _a;
  const targetIsClass = typeof target === "function";
  const proto = targetIsClass ? target.prototype : target;
  const extensionIsClass = isClass(extension);
  const home = extensionIsClass ? extension.prototype : extension;
  const members = Object.getOwnPropertyDescriptors(home);
  if (extensionIsClass) {
    Reflect.deleteProperty(members, "constructor");
  }
  const keys = Reflect.ownKeys(members);
  const holder = Object.create(proto);
  const previous = /* @__PURE__ */ new Map();
  for (const key of keys) {
    const own = Object.getOwnPropertyDescriptor(proto, key);
    previous.set(key, own);
    const original = own != null ? own : findDescriptor(Object.getPrototypeOf(proto), key);
    Object.defineProperty(holder, key, original != null ? original : { value: void 0, configurable: true, writable: true });
  }
  Object.setPrototypeOf(home, holder);
  let record = null;
  if (extensionIsClass && targetIsClass) {
    record = { target, patchClass: extension };
    const list = (_a = classPatches.get(target)) != null ? _a : [];
    list.push(record);
    classPatches.set(target, list);
    patchVersion++;
  }
  for (const key of keys) {
    let descriptor = members[key];
    if (record !== null) {
      descriptor = withLazyInit(descriptor);
    }
    Object.defineProperty(proto, key, { ...descriptor, enumerable: targetIsClass ? false : descriptor.enumerable, configurable: true });
  }
  const previousStatics = /* @__PURE__ */ new Map();
  if (extensionIsClass && targetIsClass) {
    for (const key of Reflect.ownKeys(extension)) {
      if (STATIC_SKIP.has(key) || key === Symbol.metadata) {
        continue;
      }
      previousStatics.set(key, Object.getOwnPropertyDescriptor(target, key));
      Object.defineProperty(target, key, { ...Object.getOwnPropertyDescriptor(extension, key), configurable: true });
    }
  }
  return () => {
    for (const [key, descriptor] of previous) {
      if (descriptor === void 0) {
        Reflect.deleteProperty(proto, key);
      } else {
        Object.defineProperty(proto, key, descriptor);
      }
    }
    for (const [key, descriptor] of previousStatics) {
      if (descriptor === void 0) {
        Reflect.deleteProperty(target, key);
      } else {
        Object.defineProperty(target, key, descriptor);
      }
    }
    if (record !== null) {
      const list = classPatches.get(record.target);
      if (list) {
        const index = list.indexOf(record);
        if (index !== -1) {
          list.splice(index, 1);
        }
        if (list.length === 0) {
          classPatches.delete(record.target);
        }
      }
      patchVersion++;
    }
  };
}

// src/runtime/component.ts
var construction = null;
function getConstruction() {
  return construction;
}
var INTERNALS = /* @__PURE__ */ Symbol("trame.component");
var builtinComponents = {};
var Component = class {
  constructor() {
    const ctx = construction;
    if (ctx === null) {
      throw new Error(
        "[trame] Un composant ne peut pas \xEAtre instanci\xE9 avec new : utilisez mount() ou un template."
      );
    }
    this.props = ctx.props;
    Object.defineProperty(this, INTERNALS, { value: { owner: ctx.owner, slots: ctx.slots } });
  }
};
function renderComponent(Ctor, props2, slots) {
  const owner = getOwner();
  if (owner === null) {
    throw new Error("[trame] Rendu d'un composant hors d'un scope");
  }
  const prev = construction;
  construction = { Ctor, props: props2, slots, owner, name: Ctor.name || "composant" };
  let component;
  try {
    component = runWithOwner(
      owner,
      () => untrack(() => {
        const instance = new Ctor();
        initPatches(instance);
        return instance;
      })
    );
  } finally {
    construction = prev;
  }
  const custom = Ctor.customRender;
  let roots;
  if (custom) {
    roots = runWithOwner(owner, () => untrack(() => custom(component, slots)));
  } else {
    const template = resolveTemplate(Ctor);
    const render = template.getRender("component");
    roots = runWithOwner(owner, () => untrack(() => render(component, slots, null)));
  }
  return { component, roots };
}

// src/runtime/regions.ts
var Region = class {
  constructor(anchor) {
    __publicField(this, "anchor", anchor);
  }
};
function firstOf(root) {
  return root instanceof Region ? root.firstNode() : root;
}
function lastOf(root) {
  return root instanceof Region ? root.anchor : root;
}
function itemFirst(item) {
  var _a;
  return (_a = item.placeholder) != null ? _a : firstOf(item.roots[0]);
}
function itemLast(item) {
  var _a;
  return (_a = item.placeholder) != null ? _a : lastOf(item.roots[item.roots.length - 1]);
}
function moveRange(first, last, parent, before) {
  let node = first;
  while (node !== null) {
    const next = node.nextSibling;
    parent.insertBefore(node, before);
    if (node === last) {
      break;
    }
    node = next;
  }
}
function removeRange(first, last) {
  var _a;
  let node = first;
  while (node !== null) {
    const next = node.nextSibling;
    (_a = node.parentNode) == null ? void 0 : _a.removeChild(node);
    if (node === last) {
      break;
    }
    node = next;
  }
}
function insertItem(item, parent, before) {
  moveRange(itemFirst(item), itemLast(item), parent, before);
}
function removeItem(item) {
  item.owner.dispose();
  removeRange(itemFirst(item), itemLast(item));
}
function buildItem(parent, build, boundary) {
  const owner = new Owner(parent);
  if (boundary !== void 0) {
    owner.boundary = boundary;
    owner.detached = true;
  }
  try {
    const roots = runWithOwner(owner, () => untrack(build));
    return { roots, owner, placeholder: null };
  } catch (e) {
    owner.dispose();
    throw e;
  }
}
function renderEffect(fn, loc) {
  const effect2 = new Effect(fn, getOwner(), PRIORITY_RENDER);
  effect2.loc = loc;
  effect2.run();
  return effect2;
}
function requireOwner() {
  const owner = getOwner();
  if (owner === null) {
    throw new Error("[trame] Rendu hors d'un scope");
  }
  return owner;
}
var StaticRegion = class extends Region {
  /**
   * @param shareOwner  le contenu est seul dans un bloc qui a déjà son propre scope (ligne, branche,
   *                    slot) : il l'utilise directement au lieu d'en créer un de plus.
   */
  constructor(anchor, build, loc, shareOwner = false) {
    super(anchor);
    __publicField(this, "item", null);
    const owner = requireOwner();
    try {
      this.item = shareOwner ? { roots: runWithOwner(owner, () => untrack(build)), owner, placeholder: null } : buildItem(owner, build);
      if (!owner.disposed) {
        insertItem(this.item, anchor.parentNode, anchor);
      }
    } catch (e) {
      owner.handleError(annotateError(e, loc, owner));
    }
  }
  firstNode() {
    return this.item ? itemFirst(this.item) : this.anchor;
  }
};
var NONE = /* @__PURE__ */ Symbol("none");
var SwitchRegion = class extends Region {
  constructor(anchor, keyFn, builderFor, loc) {
    super(anchor);
    __publicField(this, "builderFor", builderFor);
    __publicField(this, "loc", loc);
    __publicField(this, "current", null);
    __publicField(this, "currentKey", NONE);
    __publicField(this, "pending", null);
    __publicField(this, "owner");
    this.owner = requireOwner();
    this.owner.onCleanup(() => this.cancelPending());
    renderEffect(() => {
      const key = keyFn();
      untrack(() => this.update(key));
    }, loc);
  }
  firstNode() {
    return this.current ? itemFirst(this.current) : this.anchor;
  }
  update(key) {
    if (this.pending !== null) {
      if (this.pending.key === key) {
        return;
      }
      this.cancelPending();
    }
    if (key === this.currentKey) {
      return;
    }
    const builder = this.builderFor(key);
    const owner = this.owner;
    if (!owner.live || builder === null) {
      this.removeCurrent();
      this.currentKey = key;
      if (builder !== null) {
        try {
          this.current = buildItem(owner, builder);
          if (!owner.disposed) {
            insertItem(this.current, this.anchor.parentNode, this.anchor);
          }
        } catch (e) {
          owner.handleError(annotateError(e, this.loc, owner));
        }
      }
      return;
    }
    let item;
    const boundary = new Boundary(
      () => this.commitPending(),
      (error2) => {
        this.cancelPending();
        owner.handleError(error2);
      }
    );
    try {
      item = buildItem(owner, builder, boundary);
    } catch (e) {
      owner.handleError(annotateError(e, this.loc, owner));
      return;
    }
    if (owner.disposed) {
      boundary.cancel();
      item.owner.dispose();
      return;
    }
    this.pending = { key, item, boundary };
    boundary.done();
  }
  commitPending() {
    const pending = this.pending;
    if (pending === null) {
      return;
    }
    this.pending = null;
    this.removeCurrent();
    this.current = pending.item;
    this.currentKey = pending.key;
    insertItem(pending.item, this.anchor.parentNode, this.anchor);
    pending.item.owner.detached = false;
    if (this.owner.live) {
      pending.item.owner.activate();
    }
  }
  cancelPending() {
    const pending = this.pending;
    if (pending !== null) {
      this.pending = null;
      pending.boundary.cancel();
      pending.item.owner.dispose();
    }
  }
  removeCurrent() {
    if (this.current !== null) {
      removeItem(this.current);
      this.current = null;
    }
    this.currentKey = NONE;
  }
};
function toArray(value) {
  if (value === null || value === void 0 || value === false) {
    return [];
  }
  if (Array.isArray(value)) {
    const n = value.length;
    const result = new Array(n);
    for (let i = 0; i < n; i++) {
      result[i] = value[i];
    }
    return result;
  }
  if (typeof value === "number") {
    return Array.from({ length: value }, (_, i) => i);
  }
  if (typeof value === "string") {
    return Array.from(value);
  }
  if (typeof value === "object" && typeof value[Symbol.iterator] === "function") {
    return Array.from(value);
  }
  if (typeof value === "object") {
    return Object.values(value);
  }
  throw new Error(`[trame] t-foreach : valeur non it\xE9rable (${String(value)})`);
}
var ListRegion = class extends Region {
  constructor(anchor, listFn, keyFn, rowBuilder, loc, withIndex = true) {
    super(anchor);
    __publicField(this, "keyFn", keyFn);
    __publicField(this, "rowBuilder", rowBuilder);
    __publicField(this, "withIndex", withIndex);
    __publicField(this, "rows", []);
    __publicField(this, "owner");
    /** Sans t-key : clés de remplacement (stables) des 2e, 3e... occurrences d'une même valeur. */
    __publicField(this, "duplicates", /* @__PURE__ */ new Map());
    this.owner = requireOwner();
    renderEffect(() => {
      const items = toArray(listFn());
      const keyFn2 = this.keyFn;
      const keys = keyFn2 === null ? this.identityKeys(items) : items.map((item, i) => keyFn2(item, i));
      untrack(() => this.reconcile(items, keys));
    }, loc);
  }
  firstNode() {
    return this.rows.length ? itemFirst(this.rows[0]) : this.anchor;
  }
  /**
   * Sans t-key, la clé d'une ligne est sa valeur. Une valeur présente plusieurs fois reçoit, pour
   * chaque occurrence supplémentaire, une clé de remplacement stable d'une mise à jour à l'autre.
   */
  identityKeys(items) {
    var _a, _b, _c;
    const previous = this.duplicates;
    const next = /* @__PURE__ */ new Map();
    const counts = /* @__PURE__ */ new Map();
    const keys = new Array(items.length);
    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      const count = (_a = counts.get(item)) != null ? _a : 0;
      counts.set(item, count + 1);
      if (count === 0) {
        keys[i] = item;
        continue;
      }
      let substitutes = next.get(item);
      if (substitutes === void 0) {
        substitutes = [];
        next.set(item, substitutes);
      }
      const key = (_c = (_b = previous.get(item)) == null ? void 0 : _b[count - 1]) != null ? _c : {};
      substitutes.push(key);
      keys[i] = key;
    }
    this.duplicates = next;
    return keys;
  }
  reconcile(items, keys) {
    var _a, _b;
    const oldRows = this.rows;
    const n = items.length;
    const byKey = /* @__PURE__ */ new Map();
    for (let i = 0; i < oldRows.length; i++) {
      byKey.set(oldRows[i].key, oldRows[i]);
    }
    const parent = this.anchor.parentNode;
    const seen = /* @__PURE__ */ new Set();
    for (let i = 0; i < n; i++) {
      if (seen.has(keys[i])) {
        throw duplicateKey(keys[i]);
      }
      seen.add(keys[i]);
    }
    if (n === 0) {
      if (oldRows.length > 0 && parent.firstChild === itemFirst(oldRows[0]) && parent.lastChild === this.anchor) {
        for (const row of oldRows) {
          (_a = row.boundary) == null ? void 0 : _a.cancel();
          row.owner.dispose();
        }
        parent.textContent = "";
        parent.appendChild(this.anchor);
      } else {
        for (const row of oldRows) {
          this.removeRow(row);
        }
      }
      this.rows = [];
      return;
    }
    const newRows = new Array(n);
    const oldPositions = new Int32Array(n);
    const oldIndex = /* @__PURE__ */ new Map();
    oldRows.forEach((row, i) => oldIndex.set(row, i));
    for (let i = 0; i < n; i++) {
      const key = keys[i];
      const existing = byKey.get(key);
      if (existing !== void 0) {
        byKey.delete(key);
        existing.item.set(items[i]);
        (_b = existing.index) == null ? void 0 : _b.set(i);
        newRows[i] = existing;
        oldPositions[i] = oldIndex.get(existing);
      } else {
        newRows[i] = this.createRow(key, items[i], i);
        oldPositions[i] = -1;
        if (this.owner.disposed) {
          for (let j = 0; j <= i; j++) {
            if (oldPositions[j] === -1) {
              this.removeRow(newRows[j]);
            }
          }
          return;
        }
      }
    }
    for (const row of byKey.values()) {
      this.removeRow(row);
    }
    const stable = longestIncreasingSubsequence(oldPositions);
    let stableIndex = stable.length - 1;
    let next = this.anchor;
    for (let i = n - 1; i >= 0; i--) {
      const row = newRows[i];
      if (oldPositions[i] === -1) {
        insertItem(row, parent, next);
        this.rowInserted(row);
      } else if (stableIndex >= 0 && stable[stableIndex] === i) {
        stableIndex--;
      } else {
        insertItem(row, parent, next);
      }
      next = itemFirst(row);
    }
    this.rows = newRows;
  }
  createRow(key, value, index) {
    const item = new Signal(value);
    const indexSig = this.withIndex ? new Signal(index) : null;
    const live = this.owner.live;
    let boundary = null;
    let row;
    if (live) {
      boundary = new Boundary(
        () => this.rowReady(row),
        (error2) => this.owner.handleError(error2)
      );
    }
    const built = buildItem(this.owner, () => this.rowBuilder(item, indexSig), boundary != null ? boundary : void 0);
    row = { roots: built.roots, owner: built.owner, placeholder: null, key, item, index: indexSig, boundary };
    if (boundary !== null) {
      if (boundary.waiting) {
        row.placeholder = document.createTextNode("");
      } else {
        boundary.cancel();
        row.boundary = null;
        row.owner.boundary = this.owner.boundary;
      }
    }
    return row;
  }
  rowInserted(row) {
    if (row.boundary !== null) {
      row.boundary.done();
    } else if (row.owner.detached) {
      row.owner.detached = false;
      if (this.owner.live) {
        row.owner.activate();
      }
    }
  }
  rowReady(row) {
    const placeholder = row.placeholder;
    if (placeholder === null || row.owner.disposed) {
      return;
    }
    row.placeholder = null;
    const parent = placeholder.parentNode;
    if (parent !== null) {
      insertItem(row, parent, placeholder);
      parent.removeChild(placeholder);
    }
    row.owner.detached = false;
    if (this.owner.live) {
      row.owner.activate();
    }
  }
  removeRow(row) {
    var _a;
    (_a = row.boundary) == null ? void 0 : _a.cancel();
    removeItem(row);
  }
};
function duplicateKey(key) {
  return new Error(`[trame] t-foreach : cl\xE9 en double (${String(key)}). Utilisez t-key avec une valeur unique.`);
}
function longestIncreasingSubsequence(arr) {
  const n = arr.length;
  const predecessors = new Int32Array(n);
  const tails = [];
  for (let i = 0; i < n; i++) {
    const value = arr[i];
    if (value === -1) {
      continue;
    }
    let lo = 0;
    let hi = tails.length;
    while (lo < hi) {
      const mid = lo + hi >> 1;
      if (arr[tails[mid]] < value) {
        lo = mid + 1;
      } else {
        hi = mid;
      }
    }
    predecessors[i] = lo > 0 ? tails[lo - 1] : -1;
    tails[lo] = i;
  }
  const result = new Array(tails.length);
  let k = tails.length ? tails[tails.length - 1] : -1;
  for (let i = tails.length - 1; i >= 0; i--) {
    result[i] = k;
    k = predecessors[k];
  }
  return result;
}
var Markup = class {
  constructor(html) {
    __publicField(this, "html", html);
  }
  toString() {
    return this.html;
  }
};
function markup(html) {
  return new Markup(html);
}
var OutRegion = class extends Region {
  constructor(anchor, valueFn, loc) {
    super(anchor);
    __publicField(this, "nodes", []);
    renderEffect(() => {
      const value = valueFn();
      untrack(() => this.update(value));
    }, loc);
  }
  firstNode() {
    return this.nodes.length ? this.nodes[0] : this.anchor;
  }
  update(value) {
    const nodes = this.nodes;
    if (value === null || value === void 0 || value === false) {
      this.replace([]);
      return;
    }
    if (value instanceof Markup) {
      const tpl2 = document.createElement("template");
      tpl2.innerHTML = value.html;
      this.replace(Array.from(tpl2.content.childNodes));
      return;
    }
    if (typeof Node !== "undefined" && value instanceof Node) {
      this.replace(value.nodeType === 11 ? Array.from(value.childNodes) : [value]);
      return;
    }
    const text = String(value);
    if (nodes.length === 1 && nodes[0].nodeType === 3) {
      const node = nodes[0];
      if (node.data !== text) {
        node.data = text;
      }
      return;
    }
    this.replace([document.createTextNode(text)]);
  }
  replace(newNodes) {
    var _a;
    for (const node of this.nodes) {
      (_a = node.parentNode) == null ? void 0 : _a.removeChild(node);
    }
    const parent = this.anchor.parentNode;
    for (const node of newNodes) {
      parent.insertBefore(node, this.anchor);
    }
    this.nodes = newNodes;
  }
};

// src/runtime/dom.ts
var SVG_NS = "http://www.w3.org/2000/svg";
var MATH_NS = "http://www.w3.org/1998/Math/MathML";
var XLINK_NS = "http://www.w3.org/1999/xlink";
function buildNode(spec, parentNs) {
  if (typeof spec === "string") {
    return document.createTextNode(translateTemplateText(spec));
  }
  if (!Array.isArray(spec)) {
    return document.createTextNode(spec.r);
  }
  const [tag, attrs, children, nsCode, noTranslate] = spec;
  const ns = nsCode || parentNs;
  const el = ns === 1 ? document.createElementNS(SVG_NS, tag) : ns === 2 ? document.createElementNS(MATH_NS, tag) : document.createElement(tag);
  if (attrs) {
    for (const [name, value] of attrs) {
      const text = !noTranslate && TRANSLATABLE_ATTRIBUTES.has(name) ? translateTemplateText(value) : value;
      if (name.startsWith("xlink:")) {
        el.setAttributeNS(XLINK_NS, name, text);
      } else {
        el.setAttribute(name, text);
      }
    }
  }
  if (children) {
    const childNs = tag === "foreignObject" ? 0 : ns;
    for (const child of children) {
      el.appendChild(buildNode(child, childNs));
    }
  }
  return el;
}
function tpl(specs, fragment) {
  let template = null;
  return () => {
    if (template === null) {
      if (fragment) {
        const frag = document.createDocumentFragment();
        for (const spec of specs) {
          frag.appendChild(buildNode(spec, 0));
        }
        template = frag;
      } else {
        template = buildNode(specs[0], 0);
      }
    }
    return template.cloneNode(true);
  };
}
function toText(value) {
  if (value === null || value === void 0 || value === false) {
    return "";
  }
  return value instanceof Markup ? value.html : String(value);
}
function bindText(node, fn, loc) {
  renderEffect(() => {
    const value = fn();
    if (node.data !== value) {
      node.data = value;
    }
  }, loc);
}
function bindAttr(el, name, fn, loc) {
  let prev = void 0;
  let first = true;
  renderEffect(() => {
    const value = fn();
    if (!first && value === prev) {
      return;
    }
    first = false;
    prev = value;
    setAttribute(el, name, value);
  }, loc);
}
function setAttribute(el, name, value) {
  if (value === null || value === void 0 || value === false) {
    if (name.startsWith("xlink:")) {
      el.removeAttributeNS(XLINK_NS, name.slice(6));
    } else {
      el.removeAttribute(name);
    }
    return;
  }
  const text = value === true ? "" : String(value);
  if (name.startsWith("xlink:")) {
    el.setAttributeNS(XLINK_NS, name, text);
  } else {
    el.setAttribute(name, text);
  }
}
function bindAttrs(el, fn, loc) {
  let prev = {};
  renderEffect(() => {
    var _a;
    const value = (_a = fn()) != null ? _a : {};
    for (const name in prev) {
      if (!(name in value)) {
        el.removeAttribute(name);
      }
    }
    for (const name in value) {
      if (value[name] !== prev[name]) {
        setAttribute(el, name, value[name]);
      }
    }
    prev = { ...value };
  }, loc);
}
function bindProp(el, name, fn, loc) {
  const target = el;
  renderEffect(() => {
    const value = fn();
    if (name === "value") {
      const input = el;
      if (!sameInputValue(input.value, value)) {
        input.value = value === null || value === void 0 ? "" : String(value);
      }
    } else {
      const bool = !!value;
      if (target[name] !== bool) {
        target[name] = bool;
      }
    }
  }, loc);
}
function sameInputValue(current, wanted) {
  if (wanted === null || wanted === void 0) {
    return current === "";
  }
  if (typeof wanted === "number") {
    return current !== "" && Number(current) === wanted;
  }
  return current === String(wanted);
}
function classNames(value, out) {
  if (!value) {
    return;
  }
  if (typeof value === "string") {
    for (const name of value.split(/\s+/)) {
      if (name) {
        out.add(name);
      }
    }
  } else if (Array.isArray(value)) {
    for (const v of value) {
      classNames(v, out);
    }
  } else if (typeof value === "object") {
    for (const key in value) {
      if (value[key]) {
        classNames(key, out);
      }
    }
  }
}
function bindClass(el, fn, loc) {
  const statics = new Set(Array.from(el.classList));
  let prev = /* @__PURE__ */ new Set();
  renderEffect(() => {
    const next = /* @__PURE__ */ new Set();
    classNames(fn(), next);
    for (const name of prev) {
      if (!next.has(name) && !statics.has(name)) {
        el.classList.remove(name);
      }
    }
    for (const name of next) {
      if (!prev.has(name)) {
        el.classList.add(name);
      }
    }
    prev = next;
  }, loc);
}
function toKebab(name) {
  return name.startsWith("--") ? name : name.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase());
}
function bindStyle(el, fn, loc) {
  var _a;
  const staticStyle = (_a = el.getAttribute("style")) != null ? _a : "";
  let prev = {};
  renderEffect(() => {
    const value = fn();
    if (value === null || value === void 0 || typeof value === "string") {
      const css = value != null ? value : "";
      el.style.cssText = staticStyle && css ? `${staticStyle};${css}` : staticStyle || css;
      prev = {};
      return;
    }
    const next = {};
    for (const key in value) {
      const v = value[key];
      if (v !== null && v !== void 0 && v !== false) {
        next[toKebab(key)] = String(v);
      }
    }
    for (const key in prev) {
      if (!(key in next)) {
        el.style.removeProperty(key);
      }
    }
    for (const key in next) {
      if (next[key] !== prev[key]) {
        el.style.setProperty(key, next[key]);
      }
    }
    prev = next;
  }, loc);
}
function bindEvent(el, type, handler, modifiers, loc) {
  const kind = eventKind(type, modifiers);
  el[kind.key] = { handler, owner: getOwner(), loc };
  if (!kind.delegated) {
    el.addEventListener(type, kind.listener, kind.options);
  }
}
var NON_BUBBLING = /* @__PURE__ */ new Set([
  "focus",
  "blur",
  "mouseenter",
  "mouseleave",
  "pointerenter",
  "pointerleave",
  "load",
  "unload",
  "error",
  "abort",
  "scroll",
  "scrollend",
  "resize",
  "toggle",
  "invalid",
  "play",
  "pause",
  "ended",
  "volumechange",
  "timeupdate",
  "loadedmetadata",
  "canplay"
]);
var eventKinds = /* @__PURE__ */ new Map();
function eventKind(type, modifiers) {
  const id = `${type}|${modifiers}`;
  let kind = eventKinds.get(id);
  if (kind !== void 0) {
    return kind;
  }
  const mods = modifiers ? modifiers.split(",") : [];
  if (mods.includes("delegate")) {
    if (NON_BUBBLING.has(type) || typeof document === "undefined") {
      kind = eventKind(type, mods.filter((m) => m !== "delegate").join(","));
    } else {
      kind = delegatedKind(type, id, mods);
    }
    eventKinds.set(id, kind);
    return kind;
  }
  const prevent = mods.includes("prevent");
  const stop = mods.includes("stop");
  const self = mods.includes("self");
  const key = /* @__PURE__ */ Symbol(`trame.on.${id}`);
  const listener = function(ev) {
    const record = this[key];
    if (record === void 0 || self && ev.target !== this) {
      return;
    }
    if (prevent) {
      ev.preventDefault();
    }
    if (stop) {
      ev.stopPropagation();
    }
    runHandler(record, ev);
  };
  const options = mods.includes("capture") || mods.includes("once") || mods.includes("passive") ? { capture: mods.includes("capture"), once: mods.includes("once"), passive: mods.includes("passive") } : void 0;
  kind = { key, listener, options, delegated: false };
  eventKinds.set(id, kind);
  return kind;
}
function delegatedKind(type, id, mods) {
  const capture = mods.includes("capture");
  const passive = mods.includes("passive");
  const key = /* @__PURE__ */ Symbol(`trame.on.${id}`);
  const group = delegationGroup(type, capture, passive);
  group.kinds.push({
    key,
    prevent: mods.includes("prevent"),
    stop: mods.includes("stop"),
    self: mods.includes("self"),
    once: mods.includes("once")
  });
  return { key, listener: group.listener, options: { capture, passive }, delegated: true };
}
var delegationGroups = /* @__PURE__ */ new Map();
function delegationGroup(type, capture, passive) {
  const id = `${type}|${capture}|${passive}`;
  let group = delegationGroups.get(id);
  if (group !== void 0) {
    return group;
  }
  const kinds = [];
  const listener = (ev) => {
    var _a;
    const path = typeof ev.composedPath === "function" ? ev.composedPath() : [];
    const target = path.length ? path[0] : ev.target;
    let index = 0;
    let node = target;
    while (node !== null && node !== document) {
      const holder = node;
      let stopped = false;
      for (const kind of kinds) {
        const record = holder[kind.key];
        if (record === void 0 || kind.self && target !== node) {
          continue;
        }
        if (kind.prevent) {
          ev.preventDefault();
        }
        if (kind.once) {
          delete holder[kind.key];
        }
        runHandler(record, ev);
        stopped || (stopped = kind.stop);
      }
      if (stopped) {
        ev.stopPropagation();
        return;
      }
      node = path.length ? (_a = path[++index]) != null ? _a : null : node.parentNode;
    }
  };
  document.addEventListener(type, listener, { capture, passive });
  group = { kinds, listener };
  delegationGroups.set(id, group);
  return group;
}
function runHandler(record, ev) {
  const { owner, loc } = record;
  if (owner == null ? void 0 : owner.disposed) {
    return;
  }
  const report = (error2) => {
    if (owner !== null) {
      owner.handleActionError(annotateError(error2, loc, owner));
    } else {
      console.error(error2);
    }
  };
  try {
    runWithOwner(
      owner,
      () => batch(() => {
        const result = record.handler(ev);
        if (result !== null && typeof result === "object" && typeof result.then === "function") {
          result.then(void 0, report);
        }
      })
    );
  } catch (error2) {
    report(error2);
  }
}
function bindRef(el, setter) {
  var _a;
  setter(el);
  (_a = getOwner()) == null ? void 0 : _a.onCleanup(() => setter(null));
}

// src/runtime/helpers.ts
function resolveComponent(parent, name) {
  var _a, _b;
  const Parent = parent.constructor;
  const Ctor = (_b = (_a = Parent.components) == null ? void 0 : _a[name]) != null ? _b : builtinComponents[name];
  if (Ctor === void 0) {
    throw new Error(
      `[trame] Composant <${name}> introuvable dans ${Parent.name}. D\xE9clarez-le : static components = { ${name} };`
    );
  }
  return Ctor;
}
var emptyBlock = () => [document.createTextNode("")];
function spreadProps(explicit, spread) {
  const get = () => {
    var _a;
    return (_a = spread()) != null ? _a : {};
  };
  return new Proxy(explicit, {
    get(target, key) {
      if (key in target) {
        return Reflect.get(target, key);
      }
      return get()[key];
    },
    has(target, key) {
      return key in target || key in get();
    },
    ownKeys(target) {
      return Array.from(/* @__PURE__ */ new Set([...Reflect.ownKeys(target), ...Reflect.ownKeys(get())]));
    },
    getOwnPropertyDescriptor(target, key) {
      if (key in target) {
        return Reflect.getOwnPropertyDescriptor(target, key);
      }
      const source = get();
      return key in source ? { value: source[key], enumerable: true, configurable: true } : void 0;
    }
  });
}
var helpers = {
  tpl,
  s: toText,
  text: bindText,
  attr: bindAttr,
  attrs: bindAttrs,
  cls: bindClass,
  style: bindStyle,
  prop: bindProp,
  on: bindEvent,
  ref: bindRef,
  markup,
  _t,
  loading,
  error,
  refresh,
  computed(fn) {
    return new Computed(fn);
  },
  sw(anchor, keyFn, builders, loc) {
    return new SwitchRegion(anchor, keyFn, (i) => i >= 0 ? builders[i] : null, loc);
  },
  /** t-key hors d'une boucle : même contenu, recréé quand la clé change. */
  keyed(anchor, keyFn, builder, loc) {
    return new SwitchRegion(anchor, keyFn, () => builder, loc);
  },
  each(anchor, listFn, keyFn, rowFn, loc, withIndex) {
    return new ListRegion(anchor, listFn, keyFn, rowFn, loc, withIndex !== 0);
  },
  out(anchor, fn, loc) {
    return new OutRegion(anchor, fn, loc);
  },
  comp(anchor, parent, name, props2, slots, loc, solo) {
    return new StaticRegion(anchor, () => renderComponent(resolveComponent(parent, name), props2, slots).roots, loc, solo === 1);
  },
  dyn(anchor, parent, fn, props2, slots, loc) {
    return new SwitchRegion(
      anchor,
      () => {
        const value = fn();
        return typeof value === "string" ? resolveComponent(parent, value) : value;
      },
      (Ctor) => Ctor ? () => renderComponent(Ctor, props2, slots).roots : null,
      loc
    );
  },
  slot(anchor, slots, name, params, fallback, loc) {
    const slot = slots == null ? void 0 : slots[name];
    const builder = slot ? () => slot(params != null ? params : void 0) : fallback != null ? fallback : emptyBlock;
    return new StaticRegion(anchor, builder, loc);
  },
  call(anchor, name, component, slots, params, loc) {
    return new StaticRegion(anchor, () => getTemplate(name).getRender("call")(component, slots, params), loc);
  },
  props: spreadProps
};

// src/runtime/template.ts
var compiler = null;
function setTemplateCompiler(value) {
  compiler = value;
}
function requireCompiler(what) {
  if (compiler === null) {
    throw new Error(
      `[trame] ${what} : le compilateur de templates n'est pas inclus (trame.runtime.js). Pr\xE9compilez les templates (registerCompiled) ou utilisez trame.js.`
    );
  }
  return compiler;
}
var anonymousCount = 0;
var Template = class {
  constructor(source, name = `template_${++anonymousCount}`, base = null) {
    __publicField(this, "source", source);
    __publicField(this, "name", name);
    __publicField(this, "base", base);
    __publicField(this, "extensions", []);
    __publicField(this, "cache", /* @__PURE__ */ new Map());
    __publicField(this, "precompiled", null);
    /** Arbre fourni par un fichier de templates (registerTemplates). */
    __publicField(this, "loader", null);
    /** Templates dérivés (inheritTemplate) : ils doivent être recompilés quand celui-ci change. */
    __publicField(this, "derived", /* @__PURE__ */ new Set());
    base == null ? void 0 : base.derived.add(this);
  }
  /** Ajoute une extension (xpath) : les prochains rendus utiliseront la version étendue. */
  extend(extension) {
    this.extensions.push(extension);
    this.precompiled = null;
    this.invalidate();
  }
  /** Oublie la version compilée (la source ou ses extensions ont changé), ainsi que celle des dérivés. */
  invalidate() {
    this.cache.clear();
    for (const template of this.derived) {
      template.invalidate();
    }
  }
  /** Fonctions de rendu reçues déjà compilées. */
  setCompiled(compiled) {
    this.precompiled = compiled;
    this.invalidate();
  }
  /** Arbre XML final (base + extensions). Nécessite le compilateur. */
  getNodes() {
    return requireCompiler(`Template "${this.name}"`).nodes(this);
  }
  /** Code JS généré (débogage, précompilation). Nécessite le compilateur. */
  getCode(mode = "component") {
    return requireCompiler(`Template "${this.name}"`).code(this, mode);
  }
  getRender(mode) {
    var _a;
    let render = this.cache.get(mode);
    if (render === void 0) {
      let factory = (_a = this.precompiled) == null ? void 0 : _a[mode];
      if (factory === void 0) {
        const code = requireCompiler(`Template "${this.name}" (mode ${mode})`).code(this, mode);
        try {
          factory = new Function("$h", code);
        } catch (e) {
          throw new Error(`[trame] Template "${this.name}" : code g\xE9n\xE9r\xE9 invalide (${e.message})
${code}`);
        }
      }
      render = factory(helpers);
      this.cache.set(mode, render);
    }
    return render;
  }
};
function xml(strings, ...values) {
  let source = strings[0];
  for (let i = 0; i < values.length; i++) {
    source += String(values[i]) + strings[i + 1];
  }
  return new Template(source);
}
var namedTemplates = /* @__PURE__ */ new Map();
function namedTemplate(name) {
  let template = namedTemplates.get(name);
  if (template === void 0) {
    template = new Template("", name);
    namedTemplates.set(name, template);
  }
  return template;
}
function hasTemplate(name) {
  return namedTemplates.has(name);
}
function registerTemplate(name, source) {
  const template = typeof source === "string" ? new Template(source, name) : source;
  if (typeof source !== "string") {
    template.name = name;
  }
  namedTemplates.set(name, template);
  return template;
}
function registerCompiled(name, compiled) {
  const template = namedTemplate(name);
  template.setCompiled(compiled);
  return template;
}
function registerTemplates(content, path = "templates.xml") {
  requireCompiler(`registerTemplates("${path}")`).registerFile(content, path);
}
function getTemplate(name) {
  const template = namedTemplates.get(name);
  if (template === void 0) {
    throw new Error(`[trame] Template "${name}" introuvable`);
  }
  return template;
}
function toTemplate(target) {
  if (target instanceof Template) {
    return target;
  }
  if (typeof target === "string") {
    return getTemplate(target);
  }
  return resolveTemplate(target);
}
function extendTemplate(target, extension) {
  toTemplate(target).extend(extension);
}
function inheritTemplate(base, extension, name) {
  return new Template(extension, name, toTemplate(base));
}
function resolveTemplate(Ctor) {
  const template = Ctor.template;
  if (template === void 0) {
    throw new Error(`[trame] Le composant ${Ctor.name} n'a pas de template (static template = xml\`...\`)`);
  }
  if (typeof template === "string") {
    return getTemplate(template);
  }
  if (template.name.startsWith("template_")) {
    template.name = Ctor.name || template.name;
  }
  return template;
}

// src/runtime/compiler_setup.ts
var library = new TemplateLibrary();
var libraryTemplates = /* @__PURE__ */ new Set();
var templateCompiler = {
  nodes(template) {
    let nodes;
    if (template.loader !== null) {
      nodes = template.loader();
    } else if (template.base !== null) {
      nodes = templateCompiler.nodes(template.base);
      applyExtension(nodes, template.source, template.name, `h\xE9ritage de "${template.base.name}"`);
    } else {
      nodes = parseXML(template.source);
    }
    template.extensions.forEach((extension, i) => {
      if (typeof extension === "string") {
        applyExtension(nodes, extension, template.name, `extension n\xB0${i + 1}`);
      } else {
        applyOperations(nodes, extension, template.name);
      }
    });
    return nodes;
  },
  code(template, mode) {
    return generateCode(parseTemplate(templateCompiler.nodes(template)), mode, template.name);
  },
  registerFile(content, path) {
    const { defined, extensions } = library.addFile(content, path);
    for (const name of defined) {
      const template = namedTemplate(name);
      template.loader = () => library.resolve(name);
      libraryTemplates.add(template);
    }
    for (const { target, ops } of extensions) {
      if (library.has(target)) {
        continue;
      }
      if (!hasTemplate(target)) {
        throw new Error(`[trame] ${path} : t-inherit="${target}" vise un template inconnu (fichiers charg\xE9s dans le bon ordre ?)`);
      }
      getTemplate(target).extend(ops);
    }
    for (const template of libraryTemplates) {
      template.invalidate();
    }
  }
};
setTemplateCompiler(templateCompiler);

// src/reactivity/store.ts
var RAW = /* @__PURE__ */ Symbol("trame.raw");
var SKIP = /* @__PURE__ */ Symbol("trame.skip");
var KEYS = /* @__PURE__ */ Symbol("trame.keys");
var ITERATE = /* @__PURE__ */ Symbol("trame.iterate");
var proxies = /* @__PURE__ */ new WeakMap();
var signalsByTarget = /* @__PURE__ */ new WeakMap();
function signalFor(target, key) {
  let signals = signalsByTarget.get(target);
  if (signals === void 0) {
    signals = /* @__PURE__ */ new Map();
    signalsByTarget.set(target, signals);
  }
  let sig = signals.get(key);
  if (sig === void 0) {
    sig = new Signal(void 0, false);
    signals.set(key, sig);
  }
  return sig;
}
function track(target, key) {
  signalFor(target, key).get();
}
function trigger(target, key) {
  var _a;
  const sig = (_a = signalsByTarget.get(target)) == null ? void 0 : _a.get(key);
  sig == null ? void 0 : sig.trigger();
}
function markRaw(obj) {
  Object.defineProperty(obj, SKIP, { value: true, enumerable: false });
  return obj;
}
function toRaw(value) {
  if (value !== null && typeof value === "object") {
    const raw = value[RAW];
    if (raw !== void 0) {
      return raw;
    }
  }
  return value;
}
function canWrap(value) {
  if (value[SKIP]) {
    return false;
  }
  if (Array.isArray(value)) {
    return true;
  }
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null || value instanceof Map || value instanceof Set;
}
function reactive(value) {
  if (value === null || typeof value !== "object") {
    return value;
  }
  if (value[RAW] !== void 0) {
    return value;
  }
  const existing = proxies.get(value);
  if (existing !== void 0) {
    return existing;
  }
  if (!canWrap(value)) {
    return value;
  }
  let proxy;
  if (value instanceof Map) {
    proxy = new Proxy(value, collectionHandler);
  } else if (value instanceof Set) {
    proxy = new Proxy(value, collectionHandler);
  } else {
    proxy = new Proxy(value, Array.isArray(value) ? arrayHandler : objectHandler);
  }
  proxies.set(value, proxy);
  return proxy;
}
var hasOwn = Object.prototype.hasOwnProperty;
var objectHandler = {
  get(target, key, receiver) {
    if (key === RAW) {
      return target;
    }
    const value = Reflect.get(target, key, receiver);
    if (typeof key === "symbol") {
      return value;
    }
    track(target, key);
    return reactive(value);
  },
  set(target, key, value, receiver) {
    const had = hasOwn.call(target, key);
    const old = target[key];
    const raw = toRaw(value);
    const ok = Reflect.set(target, key, raw, receiver);
    if (!had) {
      groupWrites(() => {
        trigger(target, KEYS);
        trigger(target, key);
      });
    } else if (!Object.is(old, raw)) {
      trigger(target, key);
    }
    return ok;
  },
  deleteProperty(target, key) {
    const had = hasOwn.call(target, key);
    const ok = Reflect.deleteProperty(target, key);
    if (had) {
      groupWrites(() => {
        trigger(target, KEYS);
        trigger(target, key);
      });
    }
    return ok;
  },
  has(target, key) {
    if (typeof key !== "symbol") {
      track(target, key);
    }
    return Reflect.has(target, key);
  },
  ownKeys(target) {
    track(target, KEYS);
    return Reflect.ownKeys(target);
  }
};
var arrayMutators = /* @__PURE__ */ new Set(["push", "pop", "shift", "unshift", "splice", "sort", "reverse", "fill", "copyWithin"]);
var arrayMutatorCache = /* @__PURE__ */ new WeakMap();
var arrayHandler = {
  get(target, key, receiver) {
    if (key === RAW) {
      return target;
    }
    if (typeof key === "string" && arrayMutators.has(key)) {
      let cache = arrayMutatorCache.get(target);
      if (cache === void 0) {
        cache = /* @__PURE__ */ new Map();
        arrayMutatorCache.set(target, cache);
      }
      let fn = cache.get(key);
      if (fn === void 0) {
        const method = Array.prototype[key];
        fn = function(...args) {
          return groupWrites(() => method.apply(this, args));
        };
        cache.set(key, fn);
      }
      return fn;
    }
    if (key === "includes" || key === "indexOf" || key === "lastIndexOf") {
      const method = Array.prototype[key];
      return function(...args) {
        const result = method.apply(this, args);
        if (result === false || result === -1) {
          args[0] = toRaw(args[0]);
          return method.apply(target, args);
        }
        return result;
      };
    }
    const value = Reflect.get(target, key, receiver);
    if (typeof key === "symbol") {
      if (key === Symbol.iterator) {
        track(target, KEYS);
      }
      return value;
    }
    if (key === "length") {
      track(target, KEYS);
      return value;
    }
    if (typeof value === "function") {
      return value;
    }
    track(target, key);
    return reactive(value);
  },
  set(target, key, value, receiver) {
    const oldLength = target.length;
    const had = hasOwn.call(target, key);
    const old = target[key];
    const raw = toRaw(value);
    const ok = Reflect.set(target, key, raw, receiver);
    groupWrites(() => {
      if (key === "length") {
        for (let i = raw; i < oldLength; i++) {
          trigger(target, String(i));
        }
        if (oldLength !== target.length) {
          trigger(target, KEYS);
        }
        return;
      }
      if (!had || !Object.is(old, raw)) {
        trigger(target, key);
      }
      if (target.length !== oldLength || !had) {
        trigger(target, KEYS);
      }
    });
    return ok;
  },
  deleteProperty(target, key) {
    const had = hasOwn.call(target, key);
    const ok = Reflect.deleteProperty(target, key);
    if (had) {
      groupWrites(() => {
        trigger(target, key);
        trigger(target, KEYS);
      });
    }
    return ok;
  },
  has(target, key) {
    if (typeof key !== "symbol") {
      track(target, key);
    }
    return Reflect.has(target, key);
  },
  ownKeys(target) {
    track(target, KEYS);
    return Reflect.ownKeys(target);
  }
};
var collectionMethodCache = /* @__PURE__ */ new WeakMap();
var collectionHandler = {
  get(target, key) {
    if (key === RAW) {
      return target;
    }
    if (key === "size") {
      track(target, KEYS);
      return target.size;
    }
    const value = Reflect.get(target, key, target);
    if (typeof value !== "function") {
      return value;
    }
    let cache = collectionMethodCache.get(target);
    if (cache === void 0) {
      cache = /* @__PURE__ */ new Map();
      collectionMethodCache.set(target, cache);
    }
    let fn = cache.get(key);
    if (fn === void 0) {
      fn = wrapCollectionMethod(target, key, value);
      cache.set(key, fn);
    }
    return fn;
  }
};
function wrapCollectionMethod(target, key, method) {
  const isMap = target instanceof Map;
  switch (key) {
    case "get":
      return (k) => {
        track(target, keyOf(k));
        return reactive(target.get(toRaw(k)));
      };
    case "has":
      return (k) => {
        track(target, keyOf(k));
        return target.has(toRaw(k));
      };
    case "set":
      return (k, v) => {
        const map = target;
        const rk = toRaw(k);
        const had = map.has(rk);
        const old = map.get(rk);
        const raw = toRaw(v);
        map.set(rk, raw);
        groupWrites(() => {
          if (!had) {
            trigger(target, KEYS);
          }
          if (!had || !Object.is(old, raw)) {
            trigger(target, keyOf(rk));
            trigger(target, ITERATE);
          }
        });
        return proxies.get(target);
      };
    case "add":
      return (v) => {
        const set = target;
        const raw = toRaw(v);
        if (!set.has(raw)) {
          set.add(raw);
          groupWrites(() => {
            trigger(target, KEYS);
            trigger(target, ITERATE);
            trigger(target, keyOf(raw));
          });
        }
        return proxies.get(target);
      };
    case "delete":
      return (k) => {
        const rk = toRaw(k);
        const ok = target.delete(rk);
        if (ok) {
          groupWrites(() => {
            trigger(target, KEYS);
            trigger(target, ITERATE);
            trigger(target, keyOf(rk));
          });
        }
        return ok;
      };
    case "clear":
      return () => {
        const keys = Array.from(target.keys());
        target.clear();
        groupWrites(() => {
          trigger(target, KEYS);
          trigger(target, ITERATE);
          for (const k of keys) {
            trigger(target, keyOf(k));
          }
        });
      };
    case "forEach":
      return (callback, thisArg) => {
        track(target, ITERATE);
        const proxy = proxies.get(target);
        target.forEach((value, k) => {
          callback.call(thisArg, reactive(value), isMap ? k : reactive(k), proxy);
        });
      };
    default:
      return function(...args) {
        track(target, isMap && key === "keys" ? KEYS : ITERATE);
        const result = method.apply(target, args);
        if (result && typeof result === "object" && typeof result.next === "function") {
          const mode = isMap && key === "keys" ? "raw" : key === "entries" || isMap && key === Symbol.iterator ? "entries" : "values";
          return wrapIterator(result, mode);
        }
        return result;
      };
  }
}
function wrapIterator(it, mode) {
  return {
    next() {
      const r = it.next();
      if (r.done || mode === "raw") {
        return r;
      }
      const v = r.value;
      if (mode === "entries") {
        const [k, value] = v;
        return { done: false, value: [k, reactive(value)] };
      }
      return { done: false, value: reactive(v) };
    },
    [Symbol.iterator]() {
      return this;
    }
  };
}
var objectKeys = /* @__PURE__ */ new WeakMap();
function keyOf(k) {
  if (k !== null && (typeof k === "object" || typeof k === "function")) {
    let sym = objectKeys.get(k);
    if (sym === void 0) {
      sym = /* @__PURE__ */ Symbol();
      objectKeys.set(k, sym);
    }
    return sym;
  }
  if (typeof k === "symbol" || typeof k === "string") {
    return k;
  }
  return "\0" + typeof k + ":" + String(k);
}

// src/decorators.ts
var RESOURCES = /* @__PURE__ */ Symbol("trame.resources");
var STATE_GETTER = /* @__PURE__ */ Symbol("trame.stateGetter");
function storage(obj, key) {
  let map = obj[key];
  if (map === void 0) {
    map = /* @__PURE__ */ new Map();
    Object.defineProperty(obj, key, { value: map, enumerable: false, configurable: true });
  }
  return map;
}
function stateToJSON() {
  var _a;
  const out = {};
  for (const key of Object.keys(this)) {
    out[key] = this[key];
  }
  const chain = [];
  for (let proto = Object.getPrototypeOf(this); proto !== null && proto !== Object.prototype; proto = Object.getPrototypeOf(proto)) {
    chain.unshift(proto);
  }
  for (const proto of chain) {
    for (const name of Object.getOwnPropertyNames(proto)) {
      const getter = (_a = Object.getOwnPropertyDescriptor(proto, name)) == null ? void 0 : _a.get;
      if (getter == null ? void 0 : getter[STATE_GETTER]) {
        out[name] = this[name];
      }
    }
  }
  return out;
}
function ensureToJSON(instance) {
  const proto = Object.getPrototypeOf(instance);
  if (proto !== null && !("toJSON" in proto)) {
    Object.defineProperty(proto, "toJSON", { value: stateToJSON, enumerable: false, configurable: true, writable: true });
  }
}
var StateSignal = class extends Signal {
};
function state(target, context) {
  const key = context.name;
  const get = function() {
    const stored = target.get.call(this);
    if (stored instanceof StateSignal) {
      return stored.get();
    }
    if (getCurrentObserver() === null) {
      return stored;
    }
    const sig = new StateSignal(stored);
    target.set.call(this, sig);
    return sig.get();
  };
  get[STATE_GETTER] = true;
  return {
    init(value) {
      if (value instanceof Loader) {
        throw new Error(`[trame] "${String(key)}" : load(...) doit \xEAtre utilis\xE9 avec @resource, pas @state`);
      }
      ensureToJSON(this);
      return reactive(value);
    },
    get,
    set(value) {
      const stored = target.get.call(this);
      if (stored instanceof StateSignal) {
        stored.set(reactive(value));
      } else {
        target.set.call(this, reactive(value));
      }
    }
  };
}
var PreloadEffect = class extends Effect {
  get waitsForPending() {
    return false;
  }
  handleError() {
  }
};
function computed(first, context) {
  if (context !== void 0) {
    return computedGetter(first, context, {});
  }
  const options = first;
  return ((getter, ctx) => computedGetter(getter, ctx, options));
}
function computedGetter(getter, context, options) {
  const name = context.name;
  const cacheKey = /* @__PURE__ */ Symbol(`trame.computed.${String(name)}`);
  if (options.eager) {
    context.addInitializer(function() {
      const owner = getOwner();
      const self = this;
      scheduleMicrotask(() => {
        if (!(owner == null ? void 0 : owner.disposed)) {
          new PreloadEffect(() => void self[name], owner, PRIORITY_RESOURCE).run();
        }
      });
    });
  }
  return function() {
    let c = this[cacheKey];
    if (c === void 0) {
      const self = this;
      c = new Computed(() => getter.call(self));
      Object.defineProperty(this, cacheKey, { value: c, enumerable: false });
    }
    return c.get();
  };
}
var Loader = class {
  constructor(fetcher, options, source) {
    __publicField(this, "fetcher", fetcher);
    __publicField(this, "options", options);
    __publicField(this, "source", source);
  }
};
function load(first, second, third) {
  if (typeof second === "function") {
    return new Loader(second, third != null ? third : {}, first);
  }
  return new Loader(first, second != null ? second : {}, null);
}
function resource(_target, context) {
  const key = context.name;
  const get = (instance) => storage(instance, RESOURCES).get(key);
  return {
    init(value) {
      if (!(value instanceof Loader)) {
        throw new Error(`[trame] @resource "${String(key)}" : initialisez-la avec load(...)`);
      }
      const loader = value;
      const res = loader.source === null ? new Resource(loader.fetcher, getOwner(), loader.options) : new Resource(loader.fetcher, getOwner(), loader.options, loader.source);
      storage(this, RESOURCES).set(key, res);
      return void 0;
    },
    get() {
      var _a;
      return (_a = get(this)) == null ? void 0 : _a.read();
    },
    set(value) {
      const res = get(this);
      if (res === void 0) {
        throw new Error(`[trame] @resource "${String(key)}" non initialis\xE9e`);
      }
      res.write(value);
    }
  };
}
function effect(_method, context) {
  const name = context.name;
  context.addInitializer(function() {
    const owner = getOwner();
    const self = this;
    const start = () => {
      if (owner == null ? void 0 : owner.disposed) {
        return;
      }
      let runOwner = null;
      new Effect(
        () => {
          runOwner == null ? void 0 : runOwner.dispose();
          if (owner === null) {
            return self[name].call(self);
          }
          const scope = new Owner(owner);
          runOwner = scope;
          const cleanup = runWithOwner(scope, () => self[name].call(self));
          if (owner.live) {
            scope.activate();
          }
          return cleanup;
        },
        owner,
        PRIORITY_USER
      ).run();
    };
    if (owner !== null && !owner.live) {
      owner.onMount(start);
    } else {
      scheduleMicrotask(start);
    }
  });
}
var LazyService = class {
  constructor(Ctor, owner) {
    __publicField(this, "Ctor", Ctor);
    __publicField(this, "owner", owner);
  }
};
function parentsOf(cls) {
  const parents = [];
  for (let parent = Object.getPrototypeOf(cls); parent && parent !== Function.prototype; parent = Object.getPrototypeOf(parent)) {
    parents.push(parent);
  }
  return parents;
}
function describeService(value) {
  var _a;
  if (value instanceof LazyService) {
    return value.Ctor.name || "(classe anonyme)";
  }
  if (typeof value === "function") {
    return value.name || "(classe anonyme)";
  }
  if (value !== null && typeof value === "object") {
    return `une instance de ${((_a = value.constructor) == null ? void 0 : _a.name) || "Object"}`;
  }
  return String(value);
}
function provideOn(owner, value, key) {
  if (key !== void 0) {
    owner.provide(key, value, describeService);
    return;
  }
  if (typeof value === "function") {
    const lazy = new LazyService(value, owner);
    owner.provide(value, lazy, describeService);
    for (const parent of parentsOf(value)) {
      owner.provideInherited(parent, lazy);
    }
    return;
  }
  if (value === null || typeof value !== "object") {
    throw new Error("[trame] @provide : la valeur fournie doit \xEAtre un objet (ou pr\xE9cisez une cl\xE9 : @provide(Cle))");
  }
  const cls = value.constructor;
  owner.provide(cls, value, describeService);
  for (const parent of parentsOf(cls)) {
    owner.provideInherited(parent, value);
  }
}
var instantiating = [];
function lookupService(key) {
  const owner = getOwner();
  if (owner === null) {
    throw new Error(`[trame] @inject(${key.name}) : utilisable seulement pendant la construction d'un composant, d'un plugin ou d'un objet cr\xE9\xE9 par eux`);
  }
  let value = owner.lookup(key);
  if (value instanceof AmbiguousService) {
    throw new Error(
      `[trame] @inject(${key.name}) ambigu : plusieurs services en h\xE9ritent au m\xEAme niveau (${value.candidates.map(describeService).join(", ")}). Injectez la classe exacte, ou fournissez le service voulu sous cette cl\xE9 : @provide(Cle).`
    );
  }
  if (value instanceof LazyService) {
    const lazy = value;
    if (instantiating.includes(lazy)) {
      const cycle = [...instantiating.slice(instantiating.indexOf(lazy)), lazy].map(describeService).join(" \u2192 ");
      throw new Error(`[trame] D\xE9pendance circulaire entre services : ${cycle}`);
    }
    instantiating.push(lazy);
    let instance;
    try {
      instance = runWithOwner(
        lazy.owner,
        () => untrack(() => {
          const created = new lazy.Ctor();
          initPatches(created);
          return created;
        })
      );
    } finally {
      instantiating.pop();
    }
    lazy.owner.replaceProvided(lazy, instance);
    value = instance;
  }
  if (value === void 0) {
    throw new Error(`[trame] Aucun service ${key.name} fourni. Ajoutez-le \xE0 mount(..., { provide: [${key.name}] }) ou via @provide.`);
  }
  return value;
}
function provide(arg, context) {
  const make = (key2) => function(value) {
    const owner = getOwner();
    if (owner === null) {
      throw new Error("[trame] @provide : utilisable seulement dans un composant ou un plugin");
    }
    provideOn(owner, value, key2);
    return value;
  };
  if (context !== void 0) {
    return make(void 0);
  }
  const key = arg;
  return () => make(key);
}
function inject(key) {
  return function(_target, _context) {
    return () => lookupService(key);
  };
}

// src/runtime/app.ts
function mount(Ctor, target, options = {}) {
  return new Promise((resolve, reject) => {
    var _a, _b;
    let mounted = false;
    let destroyed = false;
    let item = null;
    const destroy = () => {
      if (destroyed) {
        return;
      }
      destroyed = true;
      boundary.cancel();
      if (item !== null && mounted) {
        removeItem(item);
      }
      owner.dispose();
    };
    const app = {
      dev: (_a = options.dev) != null ? _a : false,
      handleUncaughtError(error2) {
        if (!mounted) {
          destroy();
          reject(error2);
          return;
        }
        if (options.onError) {
          options.onError(error2);
          return;
        }
        console.error(error2);
        destroy();
      },
      handleActionError(error2) {
        if (options.onError) {
          options.onError(error2);
        } else {
          console.error(error2);
        }
      }
    };
    const owner = new Owner(null);
    owner.app = app;
    for (const service of (_b = options.provide) != null ? _b : []) {
      provideOn(owner, service);
    }
    let component;
    const boundary = new Boundary(
      () => {
        if (destroyed || item === null) {
          return;
        }
        insertItem(item, target, null);
        mounted = true;
        owner.activate();
        resolve({ component, destroy });
      },
      (error2) => app.handleUncaughtError(error2)
    );
    owner.boundary = boundary;
    const componentOwner = new Owner(owner);
    try {
      const result = runWithOwner(
        componentOwner,
        () => untrack(() => {
          var _a2;
          return renderComponent(Ctor, (_a2 = options.props) != null ? _a2 : {}, null);
        })
      );
      component = result.component;
      item = { roots: result.roots, owner: componentOwner, placeholder: null };
    } catch (error2) {
      destroy();
      reject(error2);
      return;
    }
    if (!destroyed) {
      boundary.done();
    }
  });
}

// src/props.ts
var Validator = class _Validator {
  constructor(describe2, test, isOptional = false, hasDefault = false, defaultValue = void 0, nullable = false) {
    __publicField(this, "describe", describe2);
    __publicField(this, "test", test);
    __publicField(this, "isOptional", isOptional);
    __publicField(this, "hasDefault", hasDefault);
    __publicField(this, "defaultValue", defaultValue);
    __publicField(this, "nullable", nullable);
  }
  /** Message d'erreur, ou null si la valeur est valide. */
  check(value) {
    if (value === void 0 && (this.isOptional || this.hasDefault)) {
      return null;
    }
    if (value === null && this.nullable) {
      return null;
    }
    return this.test(value);
  }
  /** Prop facultative. */
  optional() {
    return new _Validator(this.describe, this.test, true, this.hasDefault, this.defaultValue, this.nullable);
  }
  /** Valeur par défaut si la prop n'est pas passée (ou vaut undefined). */
  default(value) {
    return new _Validator(this.describe, this.test, false, true, value, this.nullable);
  }
  /** Accepte aussi null. */
  orNull() {
    return new _Validator(this.describe, this.test, this.isOptional, this.hasDefault, this.defaultValue, true);
  }
};
function simple(name, test) {
  return new Validator(name, (v) => test(v) ? null : `${name} attendu, re\xE7u ${describeValue(v)}`);
}
function describeValue(value) {
  var _a;
  if (value === null) {
    return "null";
  }
  if (Array.isArray(value)) {
    return "un tableau";
  }
  if (typeof value === "object") {
    const name = (_a = value.constructor) == null ? void 0 : _a.name;
    return name && name !== "Object" ? `une instance de ${name}` : "un objet";
  }
  return `${typeof value} (${String(value)})`;
}
var STRING = simple("string", (v) => typeof v === "string");
var NUMBER = simple("number", (v) => typeof v === "number");
var BOOLEAN = simple("boolean", (v) => typeof v === "boolean");
var FUNCTION = simple("function", (v) => typeof v === "function");
var ANY = new Validator("any", () => null);
var instanceValidators = /* @__PURE__ */ new WeakMap();
var t = {
  string: () => STRING,
  number: () => NUMBER,
  boolean: () => BOOLEAN,
  func: () => FUNCTION,
  any: () => ANY,
  instanceOf: (ctor) => {
    let validator = instanceValidators.get(ctor);
    if (validator === void 0) {
      validator = simple(ctor.name || "instance", (v) => v instanceof ctor);
      instanceValidators.set(ctor, validator);
    }
    return validator;
  },
  array: (item) => item === void 0 ? ARRAY : makeArray(item),
  object: (shape) => shape === void 0 ? OBJECT : makeObject(shape),
  literal: (...values) => new Validator(
    values.map((v) => JSON.stringify(v)).join(" | "),
    (v) => values.includes(v) ? null : `une des valeurs ${values.map((x) => JSON.stringify(x)).join(", ")} attendue, re\xE7u ${describeValue(v)}`
  ),
  or: (...validators) => new Validator(
    validators.map((x) => x.describe).join(" | "),
    (v) => validators.some((x) => x.check(v) === null) ? null : `${validators.map((x) => x.describe).join(" ou ")} attendu, re\xE7u ${describeValue(v)}`
  )
};
function makeArray(item) {
  return new Validator("array", (v) => {
    if (!Array.isArray(v)) {
      return `tableau attendu, re\xE7u ${describeValue(v)}`;
    }
    if (item) {
      for (let i = 0; i < v.length; i++) {
        const error2 = item.check(v[i]);
        if (error2) {
          return `[${i}] : ${error2}`;
        }
      }
    }
    return null;
  });
}
function makeObject(shape) {
  return new Validator("object", (v) => {
    if (v === null || typeof v !== "object" || Array.isArray(v)) {
      return `objet attendu, re\xE7u ${describeValue(v)}`;
    }
    if (shape) {
      for (const key in shape) {
        const error2 = shape[key].check(v[key]);
        if (error2) {
          return `.${key} : ${error2}`;
        }
      }
    }
    return null;
  });
}
var ARRAY = makeArray(void 0);
var OBJECT = makeObject(void 0);
function props(schema) {
  var _a, _b;
  const ctx = getConstruction();
  if (ctx === null) {
    throw new Error("[trame] props() doit \xEAtre appel\xE9 dans un champ de composant : props = props({...})");
  }
  const raw = ctx.props;
  const dev = (_b = (_a = ctx.owner.app) == null ? void 0 : _a.dev) != null ? _b : false;
  const componentName = ctx.name;
  if (dev) {
    untrack(() => {
      for (const key of Object.keys(raw)) {
        if (!Object.prototype.hasOwnProperty.call(schema, key)) {
          throw new Error(`[trame] Prop inconnue "${key}" pass\xE9e \xE0 ${componentName}. Props d\xE9clar\xE9es : ${Object.keys(schema).join(", ") || "(aucune)"}`);
        }
      }
      for (const key in schema) {
        const validator = schema[key];
        const value = raw[key];
        if (value === void 0 && !validator.isOptional && !validator.hasDefault && !(key in raw)) {
          throw new Error(`[trame] Prop obligatoire "${key}" manquante pour ${componentName}`);
        }
        const error2 = validator.check(value);
        if (error2 !== null) {
          throw new Error(`[trame] Prop "${key}" invalide pour ${componentName} : ${error2}`);
        }
      }
    });
  }
  return new Proxy(raw, handlerFor(ctx.Ctor, schema));
}
var handlers = /* @__PURE__ */ new WeakMap();
function handlerFor(Ctor, schema) {
  let handler = handlers.get(Ctor);
  if (handler !== void 0) {
    return handler;
  }
  const keys = Object.keys(schema);
  const known = new Set(keys);
  const defaults = /* @__PURE__ */ new Map();
  for (const key of keys) {
    if (schema[key].hasDefault) {
      defaults.set(key, schema[key].defaultValue);
    }
  }
  const read = (raw, key) => {
    const value = raw[key];
    return value === void 0 && defaults.has(key) ? defaults.get(key) : value;
  };
  const fail = (key) => {
    throw new TypeError(`[trame] Les props sont en lecture seule : impossible de modifier "props.${String(key)}".`);
  };
  handler = {
    get: (raw, key) => known.has(key) ? read(raw, key) : Object.prototype[key],
    has: (_, key) => known.has(key) || key in Object.prototype,
    ownKeys: () => keys,
    getOwnPropertyDescriptor: (raw, key) => known.has(key) ? { value: read(raw, key), enumerable: true, configurable: true, writable: false } : void 0,
    set: (_, key) => fail(key),
    defineProperty: (_, key) => fail(key),
    deleteProperty: (_, key) => fail(key)
  };
  handlers.set(Ctor, handler);
  return handler;
}

// src/runtime/builtins.ts
function requireOwner2() {
  const owner = getOwner();
  if (owner === null) {
    throw new Error("[trame] Rendu hors d'un scope");
  }
  return owner;
}
function detachedAnchor() {
  const fragment = document.createDocumentFragment();
  return fragment.appendChild(document.createTextNode(""));
}
var emptyRoots = () => [document.createTextNode("")];
var SuspenseRegion = class extends Region {
  constructor(anchor, slots) {
    super(anchor);
    __publicField(this, "shown", null);
    __publicField(this, "fallback", null);
    __publicField(this, "content");
    const owner = requireOwner2();
    let ready = false;
    const boundary = new Boundary(
      () => {
        ready = true;
        this.showContent(owner);
      },
      (error2) => owner.handleError(error2)
    );
    owner.onCleanup(() => boundary.cancel());
    const defaultSlot = slots == null ? void 0 : slots.default;
    this.content = buildItem(owner, defaultSlot ? () => defaultSlot() : emptyRoots, boundary);
    boundary.done();
    if (!ready) {
      const fallbackSlot = slots == null ? void 0 : slots.fallback;
      if (fallbackSlot) {
        this.fallback = buildItem(owner, () => fallbackSlot());
        this.show(this.fallback);
      }
    }
  }
  show(item) {
    insertItem(item, this.anchor.parentNode, this.anchor);
    this.shown = item;
  }
  showContent(owner) {
    if (owner.disposed || this.content.owner.disposed) {
      return;
    }
    if (this.fallback !== null) {
      removeItem(this.fallback);
      this.fallback = null;
    }
    this.show(this.content);
    this.content.owner.detached = false;
    if (owner.live) {
      this.content.owner.activate();
    }
  }
  firstNode() {
    return this.shown ? itemFirst(this.shown) : this.anchor;
  }
};
var Suspense = class extends Component {
};
__publicField(Suspense, "customRender", (_, slots) => [new SuspenseRegion(detachedAnchor(), slots)]);
var ErrorRegion = class extends Region {
  constructor(anchor, slots) {
    super(anchor);
    __publicField(this, "slots", slots);
    __publicField(this, "item", null);
    __publicField(this, "building", false);
    __publicField(this, "pendingError");
    /** Fallback affiché : les erreurs suivantes du contenu (en cours de destruction) sont ignorées. */
    __publicField(this, "failed", false);
    __publicField(this, "holder");
    __publicField(this, "owner");
    this.owner = requireOwner2();
    this.holder = this.createHolder();
    this.buildContent();
  }
  createHolder() {
    const holder = buildHolder(this.owner);
    holder.errorHandler = (error2) => {
      var _a;
      if (this.failed) {
        return true;
      }
      if (this.building) {
        (_a = this.pendingError) != null ? _a : this.pendingError = error2;
      } else {
        this.showFallback(error2);
      }
      return true;
    };
    return holder;
  }
  buildContent() {
    var _a, _b, _c;
    const slot = (_a = this.slots) == null ? void 0 : _a.default;
    this.building = true;
    this.pendingError = void 0;
    try {
      this.item = buildItem(this.holder, slot ? () => slot() : emptyRoots);
    } catch (error2) {
      (_b = this.pendingError) != null ? _b : this.pendingError = error2;
    } finally {
      this.building = false;
    }
    if (this.pendingError !== void 0) {
      const error2 = this.pendingError;
      this.pendingError = void 0;
      (_c = this.item) == null ? void 0 : _c.owner.dispose();
      this.item = null;
      this.showFallback(error2);
      return;
    }
    insertItem(this.item, this.anchor.parentNode, this.anchor);
    if (this.owner.live) {
      this.item.owner.activate();
    }
  }
  showFallback(error2) {
    var _a;
    this.failed = true;
    if (this.item !== null) {
      removeItem(this.item);
      this.item = null;
    }
    const slot = (_a = this.slots) == null ? void 0 : _a.fallback;
    const reset = () => this.reset();
    const builder = slot ? () => slot({ error: error2, reset }) : () => [document.createTextNode(String(error2))];
    this.item = buildItem(this.owner, builder);
    insertItem(this.item, this.anchor.parentNode, this.anchor);
    if (this.owner.live) {
      this.item.owner.activate();
    }
  }
  reset() {
    this.failed = false;
    if (this.item !== null) {
      removeItem(this.item);
      this.item = null;
    }
    this.buildContent();
  }
  firstNode() {
    return this.item ? itemFirst(this.item) : this.anchor;
  }
};
function buildHolder(parent) {
  return new Owner(parent);
}
var ErrorBoundary = class extends Component {
};
__publicField(ErrorBoundary, "customRender", (_, slots) => [new ErrorRegion(detachedAnchor(), slots)]);
var ErrorHandlerRegion = class extends Region {
  constructor(anchor, component, slots) {
    super(anchor);
    __publicField(this, "item");
    const holder = new Owner(requireOwner2());
    holder.actionHandler = (error2) => {
      component.props.onError(error2);
      return true;
    };
    const slot = slots == null ? void 0 : slots.default;
    this.item = buildItem(holder, slot ? () => slot() : emptyRoots);
    insertItem(this.item, this.anchor.parentNode, this.anchor);
  }
  firstNode() {
    return itemFirst(this.item);
  }
};
var ErrorHandler = class extends Component {
  constructor() {
    super(...arguments);
    __publicField(this, "props", props({ onError: t.func() }));
  }
};
// Paramètre typé Component (et non ErrorHandler) : la classe reste un ComponentClass ordinaire,
// déclarable dans `static components` comme les autres.
__publicField(ErrorHandler, "customRender", (component, slots) => [new ErrorHandlerRegion(detachedAnchor(), component, slots)]);
var PortalRegion = class extends Region {
  constructor(anchor, component, slots) {
    super(anchor);
    const owner = requireOwner2();
    const slot = slots == null ? void 0 : slots.default;
    const item = buildItem(owner, slot ? () => slot() : emptyRoots);
    owner.onMount(() => {
      const target = component.props.target;
      const el = typeof target === "string" ? document.querySelector(target) : target;
      if (!(el instanceof Element)) {
        throw new Error(`[trame] <Portal> : cible introuvable (${String(target)})`);
      }
      insertItem(item, el, null);
    });
    owner.onCleanup(() => removeRange(itemFirst(item), lastNode(item)));
  }
  firstNode() {
    return this.anchor;
  }
};
function lastNode(item) {
  const last = item.roots[item.roots.length - 1];
  return last instanceof Region ? last.anchor : last;
}
var Portal = class extends Component {
};
__publicField(Portal, "customRender", (component, slots) => [new PortalRegion(detachedAnchor(), component, slots)]);
builtinComponents.Suspense = Suspense;
builtinComponents.ErrorBoundary = ErrorBoundary;
builtinComponents.ErrorHandler = ErrorHandler;
builtinComponents.Portal = Portal;

// src/registry.ts
var insertionOrder = 0;
var Registry = class _Registry {
  constructor(name = "registry") {
    __publicField(this, "name", name);
    __publicField(this, "entries", /* @__PURE__ */ new Map());
    __publicField(this, "categories", /* @__PURE__ */ new Map());
    __publicField(this, "version", new Signal(0));
    __publicField(this, "sorted", null);
  }
  add(key, value, options = {}) {
    var _a, _b, _c;
    if (this.entries.has(key) && !options.force) {
      throw new Error(`[trame] Registre "${this.name}" : la cl\xE9 "${key}" existe d\xE9j\xE0 (utilisez { force: true } pour la remplacer)`);
    }
    const previous = this.entries.get(key);
    this.entries.set(key, {
      value,
      sequence: (_b = (_a = options.sequence) != null ? _a : previous == null ? void 0 : previous.sequence) != null ? _b : 50,
      order: (_c = previous == null ? void 0 : previous.order) != null ? _c : insertionOrder++
    });
    this.changed();
    return this;
  }
  get(key, ...rest) {
    this.version.get();
    const entry = this.entries.get(key);
    if (entry === void 0) {
      if (rest.length) {
        return rest[0];
      }
      throw new Error(`[trame] Registre "${this.name}" : cl\xE9 "${key}" introuvable`);
    }
    return entry.value;
  }
  has(key) {
    this.version.get();
    return this.entries.has(key);
  }
  remove(key) {
    if (this.entries.delete(key)) {
      this.changed();
    }
  }
  /** Valeurs triées par séquence. */
  getAll() {
    return this.getEntries().map(([, value]) => value);
  }
  /** Paires [clé, valeur] triées par séquence. */
  getEntries() {
    this.version.get();
    if (this.sorted === null) {
      this.sorted = Array.from(this.entries).sort(([, a], [, b]) => a.sequence - b.sequence || a.order - b.order).map(([key, entry]) => [key, entry.value]);
    }
    return this.sorted.slice();
  }
  get size() {
    this.version.get();
    return this.entries.size;
  }
  /** Sous-registre nommé (créé à la demande). */
  category(name) {
    let sub = this.categories.get(name);
    if (sub === void 0) {
      sub = new _Registry(`${this.name}.${name}`);
      this.categories.set(name, sub);
    }
    return sub;
  }
  changed() {
    this.sorted = null;
    this.version.set(this.version.peek() + 1);
  }
};
var registry = new Registry("registry");

// src/api.ts
var VERSION = true ? "0.2.0" : "dev";
export {
  Component,
  ErrorBoundary,
  ErrorHandler,
  Markup,
  Portal,
  Registry,
  Suspense,
  Template,
  VERSION,
  Validator,
  _t,
  batch,
  computed,
  effect,
  error,
  extendTemplate,
  getTemplate,
  inheritTemplate,
  inject,
  load,
  loading,
  markRaw,
  markup,
  mount,
  nextTick,
  patch,
  props,
  provide,
  refresh,
  registerCompiled,
  registerTemplate,
  registerTemplates,
  registry,
  resource,
  setTranslator,
  state,
  t,
  toRaw,
  untrack,
  xml
};
//# sourceMappingURL=trame.js.map
