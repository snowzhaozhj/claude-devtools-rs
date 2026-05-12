export interface DiffLine {
  type: 'added' | 'removed' | 'context'
  content: string
  oldLineNumber: number | null
  newLineNumber: number | null
}

function computeLcs(a: string[], b: string[]): number[][] {
  const m = a.length
  const n = b.length
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0))
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      dp[i][j] = a[i - 1] === b[j - 1]
        ? dp[i - 1][j - 1] + 1
        : Math.max(dp[i - 1][j], dp[i][j - 1])
    }
  }
  return dp
}

function splitDiffLines(text: string): string[] {
  if (text === '') return []
  return text.replace(/\n$/, '').split('\n')
}

export function generateDiff(oldText: string, newText: string): DiffLine[] {
  const oldLines = splitDiffLines(oldText)
  const newLines = splitDiffLines(newText)
  const dp = computeLcs(oldLines, newLines)
  const reversed: Array<Omit<DiffLine, 'oldLineNumber' | 'newLineNumber'>> = []

  let i = oldLines.length
  let j = newLines.length
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
      reversed.push({ type: 'context', content: oldLines[i - 1] })
      i--
      j--
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      reversed.push({ type: 'added', content: newLines[j - 1] })
      j--
    } else {
      reversed.push({ type: 'removed', content: oldLines[i - 1] })
      i--
    }
  }

  let oldLineNumber = 1
  let newLineNumber = 1
  return reversed.reverse().map((line) => {
    if (line.type === 'added') {
      return { ...line, oldLineNumber: null, newLineNumber: newLineNumber++ }
    }
    if (line.type === 'removed') {
      return { ...line, oldLineNumber: oldLineNumber++, newLineNumber: null }
    }
    return { ...line, oldLineNumber: oldLineNumber++, newLineNumber: newLineNumber++ }
  })
}
