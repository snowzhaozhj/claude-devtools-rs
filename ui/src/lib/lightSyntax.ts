const KEYWORDS: Record<string, Set<string>> = {
  typescript: new Set([
    'import', 'export', 'from', 'const', 'let', 'var', 'function', 'class', 'interface',
    'type', 'enum', 'return', 'if', 'else', 'for', 'while', 'do', 'switch', 'case',
    'break', 'continue', 'try', 'catch', 'finally', 'throw', 'new', 'this', 'super',
    'extends', 'implements', 'async', 'await', 'public', 'private', 'protected', 'static',
    'readonly', 'abstract', 'as', 'typeof', 'instanceof', 'in', 'of', 'keyof', 'void',
    'never', 'unknown', 'any', 'null', 'undefined', 'true', 'false', 'default',
  ]),
  javascript: new Set([
    'import', 'export', 'from', 'const', 'let', 'var', 'function', 'class', 'return',
    'if', 'else', 'for', 'while', 'do', 'switch', 'case', 'break', 'continue', 'try',
    'catch', 'finally', 'throw', 'new', 'this', 'super', 'extends', 'async', 'await',
    'typeof', 'instanceof', 'in', 'of', 'void', 'null', 'undefined', 'true', 'false',
    'default',
  ]),
  python: new Set([
    'import', 'from', 'as', 'def', 'class', 'return', 'if', 'elif', 'else', 'for',
    'while', 'break', 'continue', 'try', 'except', 'finally', 'raise', 'with', 'pass',
    'lambda', 'yield', 'global', 'nonlocal', 'assert', 'and', 'or', 'not', 'in', 'is',
    'True', 'False', 'None', 'async', 'await', 'self', 'cls',
  ]),
  rust: new Set([
    'fn', 'let', 'mut', 'const', 'static', 'struct', 'enum', 'impl', 'trait', 'pub',
    'mod', 'use', 'crate', 'self', 'super', 'where', 'for', 'loop', 'while', 'if',
    'else', 'match', 'return', 'break', 'continue', 'move', 'ref', 'as', 'in', 'async',
    'await', 'dyn', 'true', 'false', 'type', 'extern',
  ]),
  go: new Set([
    'package', 'import', 'func', 'var', 'const', 'type', 'struct', 'interface', 'map',
    'chan', 'go', 'defer', 'return', 'if', 'else', 'for', 'range', 'switch', 'case',
    'default', 'break', 'continue', 'fallthrough', 'select', 'nil', 'true', 'false',
  ]),
  ruby: new Set([
    'def', 'class', 'module', 'end', 'do', 'if', 'elsif', 'else', 'unless', 'while',
    'until', 'for', 'in', 'begin', 'rescue', 'ensure', 'raise', 'return', 'yield',
    'require', 'include', 'extend', 'self', 'super', 'nil', 'true', 'false',
  ]),
  php: new Set([
    'function', 'class', 'interface', 'trait', 'extends', 'implements', 'namespace',
    'use', 'public', 'private', 'protected', 'static', 'abstract', 'final', 'const',
    'var', 'new', 'return', 'if', 'elseif', 'else', 'for', 'foreach', 'while', 'do',
    'switch', 'case', 'break', 'continue', 'default', 'try', 'catch', 'finally',
    'throw', 'as', 'echo', 'print', 'true', 'false', 'null', 'array', 'self', 'this',
  ]),
  sql: new Set([
    'SELECT', 'FROM', 'WHERE', 'INSERT', 'INTO', 'UPDATE', 'SET', 'DELETE', 'CREATE',
    'ALTER', 'DROP', 'TABLE', 'INDEX', 'VIEW', 'DATABASE', 'JOIN', 'INNER', 'LEFT',
    'RIGHT', 'OUTER', 'FULL', 'CROSS', 'ON', 'AND', 'OR', 'NOT', 'IN', 'EXISTS',
    'BETWEEN', 'LIKE', 'IS', 'NULL', 'AS', 'ORDER', 'BY', 'GROUP', 'HAVING', 'LIMIT',
    'OFFSET', 'UNION', 'ALL', 'DISTINCT', 'COUNT', 'SUM', 'AVG', 'MIN', 'MAX', 'CASE',
    'WHEN', 'THEN', 'ELSE', 'END', 'BEGIN', 'COMMIT', 'ROLLBACK', 'TRANSACTION',
    'PRIMARY', 'KEY', 'FOREIGN', 'REFERENCES', 'CONSTRAINT', 'DEFAULT', 'VALUES',
    'TRUE', 'FALSE', 'INTEGER', 'VARCHAR', 'TEXT', 'BOOLEAN', 'DATE', 'TIMESTAMP',
  ]),
};

KEYWORDS.tsx = KEYWORDS.typescript;
KEYWORDS.jsx = KEYWORDS.javascript;
KEYWORDS.ts = KEYWORDS.typescript;
KEYWORDS.js = KEYWORDS.javascript;

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function span(className: string, text: string): string {
  return `<span class="${className}">${escapeHtml(text)}</span>`;
}

function isHashCommentLanguage(language: string): boolean {
  return ['python', 'bash', 'shell', 'sh', 'zsh', 'fish', 'r', 'ruby', 'php'].includes(language);
}

export function lightHighlightLine(line: string, language: string): string {
  const keywords = KEYWORDS[language] ?? new Set<string>();
  if (keywords.size === 0 && !['json', 'css', 'html', 'xml', 'bash', 'shell', 'sh', 'markdown'].includes(language)) {
    return escapeHtml(line);
  }

  let html = '';
  let currentPos = 0;

  while (currentPos < line.length) {
    const remaining = line.slice(currentPos);

    const quote = remaining[0];
    if (quote === '"' || quote === "'" || quote === '`') {
      const endQuote = remaining.indexOf(quote, 1);
      if (endQuote !== -1) {
        const str = remaining.slice(0, endQuote + 1);
        html += span('syntax-string', str);
        currentPos += str.length;
        continue;
      }
    }

    if (remaining.startsWith('//')) {
      html += span('syntax-comment', remaining);
      break;
    }

    if (isHashCommentLanguage(language) && remaining.startsWith('#')) {
      html += span('syntax-comment', remaining);
      break;
    }

    if (language === 'sql' && remaining.startsWith('--')) {
      html += span('syntax-comment', remaining);
      break;
    }

    const numberMatch = /^(\d+\.?\d*)/.exec(remaining);
    if (numberMatch && (currentPos === 0 || /\W/.test(line[currentPos - 1] ?? ''))) {
      html += span('syntax-number', numberMatch[1]);
      currentPos += numberMatch[1].length;
      continue;
    }

    const wordMatch = /^([a-zA-Z_$][a-zA-Z0-9_$]*)/.exec(remaining);
    if (wordMatch) {
      const word = wordMatch[1];
      if (keywords.has(word) || (language === 'sql' && keywords.has(word.toUpperCase()))) {
        html += span('syntax-keyword', word);
      } else if ((word[0]?.toUpperCase() ?? '') === word[0] && word.length > 1) {
        html += span('syntax-type', word);
      } else {
        html += escapeHtml(word);
      }
      currentPos += word.length;
      continue;
    }

    const opMatch = /^([=<>!+\-*/%&|^~?:;,.{}()[\]])/.exec(remaining);
    if (opMatch) {
      html += span('syntax-operator', opMatch[1]);
      currentPos += 1;
      continue;
    }

    html += escapeHtml(remaining[0]);
    currentPos += 1;
  }

  return html;
}

export function escapeCodeLine(line: string): string {
  return escapeHtml(line);
}
