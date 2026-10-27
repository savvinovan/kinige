/**
 * Теги записи для `MockApi`: упрощённая копия правил `kinige_domain::tags` (настоящий разбор — в Rust). Тег — `#` и следом
 * буквы любого алфавита, цифры, `_`, не только цифры; регистр не важен; не тег код, ссылки, `#` после буквы/цифры/`_`.
 */
export function mockTags(body: string): string[] {
  const text = body
    .replace(/^(```|~~~)[\s\S]*?(?:^\1|$(?![\s\S]))/gm, " ")
    .replace(/`[^`\n]*`/g, " ")
    .replace(/\[[^\]\n]*\]\([^)\n]*\)/g, " ")
    .replace(/<[^>\n]+>/g, " ")
    .replace(/(?:https?:\/\/|mailto:|www\.)\S+/gi, " ");
  const tags: string[] = [];
  for (const match of text.matchAll(/(?<![\p{L}\p{N}_#&\\])#([\p{L}\p{N}_]{1,64})/gu)) {
    const tag = (match[1] ?? "").toLowerCase();
    if (/^\d+$/.test(tag) || tags.includes(tag)) continue;
    tags.push(tag);
  }
  return tags;
}
