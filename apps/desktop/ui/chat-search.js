/* WA Desk frontend: chat-search. Loaded in order by index.html; all files share the classic-script global scope. */

/* --------------------------------------------------------- in-chat search */

let chatSearchHits = [];
let chatSearchIndex = -1;

function clearChatSearchMarks() {
  for (const node of $("messages").querySelectorAll(".bubble.search-hit, .bubble.search-active")) {
    node.classList.remove("search-hit", "search-active");
  }
  if (window.CSS && CSS.highlights) CSS.highlights.delete("chat-search");
  chatSearchHits = [];
  chatSearchIndex = -1;
}

function updateChatSearchCount() {
  const count = $("chat-search-count");
  const total = chatSearchHits.length;
  count.textContent = total
    ? `${chatSearchIndex + 1}/${total}`
    : $("chat-search-input").value.trim()
      ? "0/0"
      : "";
  $("chat-search-prev").disabled = total === 0;
  $("chat-search-next").disabled = total === 0;
}

function scrollToChatSearchHit(index) {
  if (!chatSearchHits.length) return;
  chatSearchIndex = (index + chatSearchHits.length) % chatSearchHits.length;
  for (const node of chatSearchHits) node.classList.remove("search-active");
  const active = chatSearchHits[chatSearchIndex];
  active.classList.add("search-active");
  active.scrollIntoView({ block: "center", behavior: "smooth" });
  updateChatSearchCount();
}

function runChatSearch(query, keepIndex = false) {
  const previous = keepIndex ? chatSearchIndex : -1;
  clearChatSearchMarks();
  const needle = query.trim().toLowerCase();
  if (!needle) {
    updateChatSearchCount();
    return;
  }

  const ranges = [];
  for (const bubble of $("messages").querySelectorAll(".bubble[data-id]")) {
    let matched = false;
    for (const body of bubble.querySelectorAll(".body")) {
      const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT);
      let node;
      while ((node = walker.nextNode())) {
        const value = (node.nodeValue || "").toLowerCase();
        let from = 0;
        while (true) {
          const at = value.indexOf(needle, from);
          if (at < 0) break;
          matched = true;
          if (typeof Highlight === "function") {
            const range = document.createRange();
            range.setStart(node, at);
            range.setEnd(node, at + needle.length);
            ranges.push(range);
          }
          from = at + needle.length;
        }
      }
    }
    if (matched) {
      bubble.classList.add("search-hit");
      chatSearchHits.push(bubble);
    }
  }

  if (window.CSS && CSS.highlights && ranges.length) {
    try {
      CSS.highlights.set("chat-search", new Highlight(...ranges));
    } catch {
      /* the highlight API can reject cross-node ranges */
    }
  }

  updateChatSearchCount();
  if (chatSearchHits.length) {
    scrollToChatSearchHit(previous >= 0 ? Math.min(previous, chatSearchHits.length - 1) : 0);
  }
}

function openChatSearch() {
  if (!state.active) return;
  $("chat-search-bar").hidden = false;
  const input = $("chat-search-input");
  input.focus();
  input.select();
}

function closeChatSearch() {
  $("chat-search-bar").hidden = true;
  $("chat-search-input").value = "";
  clearChatSearchMarks();
  updateChatSearchCount();
}

function toggleChatSearch() {
  if ($("chat-search-bar").hidden) openChatSearch();
  else closeChatSearch();
}

