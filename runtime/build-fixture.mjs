import { readFileSync, writeFileSync } from 'node:fs';

const runtimeDirectory = new URL('./', import.meta.url);
const fixtureUrl = new URL('fixture.svg', runtimeDirectory);
const style = readFileSync(new URL('svugy.css', runtimeDirectory), 'utf8');
const script = readFileSync(new URL('svugy.js', runtimeDirectory), 'utf8');
const fixture = readFileSync(fixtureUrl, 'utf8');

const styleBlock = /(<style>\/\*<!\[CDATA\[\*\/)[\s\S]*?(\/\*\]\]>\*\/<\/style>)/;
const scriptBlock = /(<script>\/\/<!\[CDATA\[\n)[\s\S]*?(\n\/\/\]\]><\/script>)/;
const isFixtureShaped = styleBlock.test(fixture) && scriptBlock.test(fixture);
if (!isFixtureShaped) {
  throw new Error('fixture.svg lost its CDATA-wrapped <style> or <script> block');
}
const styledFixture = fixture.replace(styleBlock, (_match, opening, closing) => `${opening}${style}${closing}`);
const inlinedFixture = styledFixture.replace(scriptBlock, (_match, opening, closing) => `${opening}${script}${closing}`);
writeFileSync(fixtureUrl, inlinedFixture);
