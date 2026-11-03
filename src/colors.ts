/** 把主题里的 #RGB / #RRGGBB / #RRGGBBAA 转成 CSS 可用颜色 */
export function toCssColor(value: string | undefined, fallback: string): string {
  const v = (value ?? "").trim();
  const m = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(v);
  if (m) {
    if (m[2]) {
      const r = parseInt(m[1].slice(0, 2), 16);
      const g = parseInt(m[1].slice(2, 4), 16);
      const b = parseInt(m[1].slice(4, 6), 16);
      const a = (parseInt(m[2], 16) / 255).toFixed(3);
      return `rgba(${r}, ${g}, ${b}, ${a})`;
    }
    return v;
  }
  if (/^#([0-9a-f]{3})$/i.test(v)) return v;
  return fallback;
}
