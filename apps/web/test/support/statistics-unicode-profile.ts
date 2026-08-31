export const STATISTICS_COUNTING_PROFILE = "storyos.statistics.unicode-16.0.0.v1";

export function countStoredText(text: string): { word_count: number; character_count: number } {
  let characterCount = 0;
  let wordCount = 0;
  let inWord = false;
  for (const scalar of text) {
    characterCount += 1;
    if (isUnicode16WhiteSpace(scalar)) {
      inWord = false;
    } else if (!inWord) {
      wordCount += 1;
      inWord = true;
    }
  }
  return { word_count: wordCount, character_count: characterCount };
}

function isUnicode16WhiteSpace(scalar: string): boolean {
  const code = scalar.codePointAt(0);
  if (code === undefined) return false;
  return (code >= 0x09 && code <= 0x0d)
    || code === 0x20
    || code === 0x85
    || code === 0xa0
    || code === 0x1680
    || (code >= 0x2000 && code <= 0x200a)
    || code === 0x2028
    || code === 0x2029
    || code === 0x202f
    || code === 0x205f
    || code === 0x3000;
}
