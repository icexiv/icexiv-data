import styles from "./style.js";

const labels = { ready: "可用", partial: "部分", reference: "资料" };

// Text-only data and local book routes; no HTML from the source is evaluated.
export function parseCatalog(source) {
  const groups = JSON.parse(source);
  if (!Array.isArray(groups) || groups.length > 20) throw new Error("分组数据无效");
  let count = 0;
  for (const group of groups) {
    if (typeof group.title !== "string" || !Array.isArray(group.items)) throw new Error("分组数据无效");
    for (const item of group.items) {
      if (++count > 100 || !Object.hasOwn(labels, item.status)
        || ![item.name, item.summary, item.limit].every(value => typeof value === "string")
        || typeof item.route !== "string" || !/^[a-z0-9-]+\/$/.test(item.route)) {
        throw new Error("格式条目无效");
      }
    }
  }
  return groups;
}

export default class SupportCatalog extends HTMLElement {
  static observedAttributes = ["notist-source"];
  #selected = "all";
  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }
  connectedCallback() { this.render(); }
  attributeChangedCallback() { if (this.isConnected) this.render(); }
  render() {
    const root = this.shadowRoot;
    const doc = this.ownerDocument;
    const make = (tag, text, className) => {
      const element = doc.createElement(tag);
      if (text !== undefined) element.textContent = text;
      if (className) element.className = className;
      return element;
    };
    root.replaceChildren(make("style", styles));
    let groups;
    try { groups = parseCatalog(this.getAttribute("notist-source") ?? "[]"); }
    catch {
      const error = make("p", "支持目录数据无法读取。请查阅需求覆盖清单。", "error");
      error.setAttribute("role", "alert");
      root.append(error);
      return;
    }
    const base = doc.querySelector('link[rel="home"]')?.href ?? new URL("../", doc.baseURI).href;
    const filters = make("div", undefined, "filters");
    filters.setAttribute("role", "group");
    filters.setAttribute("aria-label", "按支持状态筛选");
    const sections = [];
    const apply = () => {
      for (const button of filters.children) button.setAttribute("aria-pressed", String(button.dataset.status === this.#selected));
      for (const section of sections) {
        for (const card of section.querySelectorAll("article")) card.hidden = this.#selected !== "all" && card.dataset.status !== this.#selected;
        section.hidden = ![...section.querySelectorAll("article")].some(card => !card.hidden);
      }
    };
    for (const [status, label] of Object.entries({ all: "全部", ...labels })) {
      const count = groups.flatMap(group => group.items).filter(item => status === "all" || item.status === status).length;
      const button = make("button", `${label} ${count}`);
      button.type = "button";
      button.dataset.status = status;
      button.addEventListener("click", () => { this.#selected = status; apply(); });
      filters.append(button);
    }
    root.append(filters);
    for (const group of groups) {
      const section = make("section");
      section.setAttribute("aria-label", group.title);
      section.append(make("h2", group.title));
      const grid = make("div", undefined, "grid");
      for (const item of group.items) {
        const card = make("article");
        card.dataset.status = item.status;
        const header = make("header");
        const title = make("h3");
        const link = make("a", item.name);
        link.href = new URL(item.route, base).href;
        title.append(link);
        header.append(title, make("span", labels[item.status], `badge ${item.status}`));
        card.append(header, make("p", item.summary));
        const details = make("details");
        details.append(make("summary", "范围与限制"), make("p", item.limit));
        const reference = make("a", "格式参考 →");
        reference.href = link.href;
        details.append(reference);
        card.append(details);
        grid.append(card);
      }
      section.append(grid);
      sections.push(section);
      root.append(section);
    }
    apply();
  }
}
