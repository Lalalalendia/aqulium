import { Buffer } from 'node:buffer';

const mb = 1_000_000;
const asciiSentence =
  'Markdown paragraph about tables, lists, references [[note]], normal editing and saving in Aquilum.';
const unicodeSentence =
  'Обычный текст заметки: кириллица, форматирование и поиск. English text and emoji 🌍.';

/** A deterministic, explicit UTF-8 workload. No random inputs. */
export function fixtureOf(profile, targetMb) {
  if (!['mixed', 'dense', 'ascii'].includes(profile)) throw new Error('unknown profile');
  if (![1, 5, 20].includes(targetMb)) throw new Error('unknown size');
  const lines = [];
  let booksInBlock = 0;
  let quotesInBlock = 0;
  for (let i = 0; i < 128; i++) {
    let line;
    if (profile === 'dense' && i % 4 === 0) {
      line = '\t>  [!BoOk] [[Page]] Some bookmarked content';
      booksInBlock++;
    } else if (profile === 'dense' && i % 7 === 0) {
      line = '  > \t[!QUoTE] Cited text [1](aquilum-reader:cfi=xyz)';
      quotesInBlock++;
    } else if (i % 29 === 0) {
      line = '  >  [!book] [[Chapter 1]]';
      booksInBlock++;
    } else if (i % 43 === 0) {
      line = '> [!quote] Цитата [1](aquilum-reader:cfi=abc)';
      quotesInBlock++;
    } else if (i % 18 === 0) {
      line = '  > Ordinary blockquote but not a recognized callout';
    } else if (i % 31 === 0) {
      line = '| Heading | Value |\n| --- | --- |\n| Row | 1 |';
    } else if (i % 37 === 0) {
      line = 'Prose about > [!book] in the middle of text: this is NOT a callout';
    } else {
      line = 'Paragraph ' + i + ': ' + (profile === 'ascii' ? asciiSentence : unicodeSentence)
        + (i % 3 === 0 ? '\t \t' : '  ');
    }
    lines.push(line);
  }
  const block = lines.join('\n') + '\n';
  const blockBytes = Buffer.byteLength(block);
  const repeats = Math.ceil(targetMb * mb / blockBytes);
  const text = block.repeat(repeats);
  const expectedBooks = booksInBlock * repeats;
  const expectedQuotes = quotesInBlock * repeats;
  if (expectedBooks > 65535 || expectedQuotes > 65535) throw new Error('benchmark count overflow');
  return {
    text,
    profile,
    requestedMb: targetMb,
    utf8Bytes: Buffer.byteLength(text),
    utf16Units: text.length,
    books: expectedBooks,
    quotes: expectedQuotes,
    encodedResult: expectedBooks * 65536 + expectedQuotes,
  };
}
