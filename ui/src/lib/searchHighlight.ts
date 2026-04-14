const MARK_ATTR = "data-search-match";
const CURRENT_ATTR = "data-search-current";

/**
 * 在 container 内的文本节点中高亮所有 query 匹配项，返回匹配总数。
 * 调用前应先 clearHighlights。
 */
export function highlightMatches(container: HTMLElement, query: string): number {
  if (!query) return 0;

  const lowerQuery = query.toLowerCase();
  const SKIP_TAGS = new Set(["PRE", "CODE", "STYLE", "SCRIPT"]);
  const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement;
      if (parent && SKIP_TAGS.has(parent.tagName)) return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  const textNodes: Text[] = [];

  let node: Text | null;
  while ((node = walker.nextNode() as Text | null)) {
    if (node.nodeValue && node.nodeValue.toLowerCase().includes(lowerQuery)) {
      textNodes.push(node);
    }
  }

  let total = 0;
  for (const textNode of textNodes) {
    const text = textNode.nodeValue!;
    const lowerText = text.toLowerCase();
    const frag = document.createDocumentFragment();
    let lastIndex = 0;
    let pos = lowerText.indexOf(lowerQuery, lastIndex);

    while (pos !== -1) {
      if (pos > lastIndex) {
        frag.appendChild(document.createTextNode(text.slice(lastIndex, pos)));
      }
      const mark = document.createElement("mark");
      mark.setAttribute(MARK_ATTR, String(total));
      mark.textContent = text.slice(pos, pos + query.length);
      frag.appendChild(mark);
      total++;
      lastIndex = pos + query.length;
      pos = lowerText.indexOf(lowerQuery, lastIndex);
    }

    if (lastIndex < text.length) {
      frag.appendChild(document.createTextNode(text.slice(lastIndex)));
    }

    textNode.parentNode!.replaceChild(frag, textNode);
  }

  return total;
}

/** 移除 container 内所有搜索高亮 <mark>，恢复原始文本 */
export function clearHighlights(container: HTMLElement): void {
  const marks = container.querySelectorAll<HTMLElement>(`mark[${MARK_ATTR}]`);
  for (const mark of marks) {
    const parent = mark.parentNode;
    if (!parent) continue;
    const text = document.createTextNode(mark.textContent || "");
    parent.replaceChild(text, mark);
    parent.normalize();
  }
}

/** 滚动到第 index 个匹配项并标记为当前项 */
export function scrollToMatch(container: HTMLElement, index: number): void {
  // 清除之前的 current 标记
  const prev = container.querySelector<HTMLElement>(`mark[${CURRENT_ATTR}]`);
  if (prev) prev.removeAttribute(CURRENT_ATTR);

  const target = container.querySelector<HTMLElement>(`mark[${MARK_ATTR}="${index}"]`);
  if (target) {
    target.setAttribute(CURRENT_ATTR, "");
    target.scrollIntoView({ block: "center", behavior: "smooth" });
  }
}
