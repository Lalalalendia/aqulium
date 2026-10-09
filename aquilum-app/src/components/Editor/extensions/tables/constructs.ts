import type { ChangeDesc, Text } from '@codemirror/state';
import { parseMarkdownTable, serializeMarkdownTable } from './markdown';
import type { TableModel } from './model';
import { isSeparatorRow, splitTableRow } from './rows';

export interface TableRange {
    from: number;
    contentTo: number;
    to: number;
    model: TableModel;
}

export function sanitizeCellValue(value: string): string {
    return value
        .replace(/\r\n?/g, '\n')
        .replace(/\n/g, ' ')
        .replace(/\|/g, '∣')
        .trimEnd();
}

export function serializeTable(model: TableModel): string {
    return serializeMarkdownTable({
        ...model,
        cells: model.cells.map((row) => row.map(sanitizeCellValue)),
    });
}

function lineLooksLikeTableRow(line: string): boolean {
    return line.includes('|') && splitTableRow(line) != null;
}

function findCompleteTablesInLines(doc: Text, fromLine: number, toLine: number): TableRange[] {
    const ranges: TableRange[] = [];
    let lineNum = Math.max(1, fromLine);

    while (lineNum <= toLine && lineNum <= doc.lines) {
        const startLine = doc.line(lineNum);
        if (!lineLooksLikeTableRow(startLine.text) || isSeparatorRow(startLine.text)) {
            lineNum += 1;
            continue;
        }
        if (lineNum + 1 > doc.lines) break;
        if (!isSeparatorRow(doc.line(lineNum + 1).text)) {
            lineNum += 1;
            continue;
        }

        let endLineNum = lineNum + 1;
        while (endLineNum + 1 <= toLine && endLineNum + 1 <= doc.lines) {
            const next = doc.line(endLineNum + 1);
            if (!lineLooksLikeTableRow(next.text) || isSeparatorRow(next.text)) break;
            if (
                endLineNum + 2 <= doc.lines
                && isSeparatorRow(doc.line(endLineNum + 2).text)
            ) {
                break;
            }
            endLineNum += 1;
        }

        if (endLineNum < lineNum + 2) {
            lineNum += 1;
            continue;
        }

        const from = startLine.from;
        const metadataLine = endLineNum < doc.lines ? doc.line(endLineNum + 1) : null;
        if (metadataLine?.text.trim().startsWith('<!--q-table:')) endLineNum += 1;
        const contentTo = doc.line(endLineNum).to;
        const to = endLineNum < doc.lines ? doc.line(endLineNum + 1).from : doc.length;
        const model = parseMarkdownTable(doc.sliceString(from, contentTo));
        if (model) {
            ranges.push({ from, contentTo, to, model });
            lineNum = endLineNum + 1;
            continue;
        }
        lineNum += 1;
    }

    return ranges;
}

// CodeMirror Text is an immutable persistent rope. Holding parsed tables by
// Text identity is safe and allows transaction filters, previews and caret
// guards to share the same parse without retaining obsolete documents.
const parsedTables = new WeakMap<Text, TableRange[]>();

export function findTablesInDoc(doc: Text): TableRange[] {
    const known = parsedTables.get(doc);
    if (known) return known;
    const tables = doc.length === 0 ? [] : findCompleteTablesInLines(doc, 1, doc.lines);
    parsedTables.set(doc, tables);
    return tables;
}

// A table needs a row containing | plus a separator row. Before reusing an
// earlier parse, inspect both sides of each change (including line joins) and
// the two surrounding lines. Any pipe or layout metadata forces a full parse.
// False positives only cost CPU; false negatives could hide a new table.
function tableSyntaxNear(doc: Text, from: number, to: number): boolean {
    const start = doc.lineAt(Math.min(from, doc.length)).number;
    const end = doc.lineAt(Math.min(to, doc.length)).number;
    const first = Math.max(1, start - 2);
    const last = Math.min(doc.lines, end + 2);
    for (let i = first; i <= last; i++) {
        const line = doc.line(i).text;
        if (line.includes('|') || line.trimStart().startsWith('<!--q-table:')) {
            return true;
        }
    }
    return false;
}

/**
 * Map cached table ranges through a transaction when no edit can change table
 * syntax or ownership. Edits inside or adjacent to tables, and any newly
 * possible table syntax, still use the original full parser.
 *
 * Reusing model objects is safe because the corresponding Markdown text did
 * not change. Widget positional handles are rebuilt by the decoration field.
 */
export function findTablesAfterChanges(
    oldDoc: Text,
    newDoc: Text,
    changes: ChangeDesc,
): TableRange[] {
    const oldTables = findTablesInDoc(oldDoc);
    let needsScan = false;
    changes.iterChangedRanges((fromA, toA, fromB, toB) => {
        if (needsScan) return;
        if (
            tableSyntaxNear(oldDoc, fromA, toA)
            || tableSyntaxNear(newDoc, fromB, toB)
            || oldTables.some((table) =>
                fromA <= table.to && toA >= table.from
            )
        ) {
            needsScan = true;
        }
    });
    if (needsScan) return findTablesInDoc(newDoc);

    // An edit outside all table ranges changes only document coordinates. For
    // an append after the final table, there is nothing to move.
    const mapped = oldTables.map((table) => ({
        ...table,
        from: changes.mapPos(table.from, 1),
        contentTo: changes.mapPos(table.contentTo, 1),
        to: changes.mapPos(table.to, 1),
    }));
    parsedTables.set(newDoc, mapped);
    return mapped;
}

export function tableOwnsDocEnd(doc: Text, table: TableRange): boolean {
    if (table.contentTo <= table.from || doc.length === 0) return false;
    return doc.lineAt(table.contentTo - 1).number === doc.lines;
}
