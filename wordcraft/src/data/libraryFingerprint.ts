/**
 * 词库指纹。用内容而非词数——同样多的词换一批内容也该触发重导，
 * 而词数相等就静静跳过是最难察觉的一种失效。
 *
 * FNV-1a：够短、无依赖，碰撞概率对「判断文件变没变」完全够用。
 */
export function fingerprintOf(text: string): string {
  let h = 0x811c9dc5
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i)
    h = Math.imul(h, 0x01000193) >>> 0
  }
  return `${text.length}-${h.toString(16)}`
}
