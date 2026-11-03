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

async function updateChatSearchCount() {
  const count = $("chat-search-count");
  const label = await invoke("ui_search_label", {
    index: chatSearchIndex,
    total: chatSearchHits.length,
    query: $("chat-search-input").value,
  }).catch(() => ({ text: "", disabled: true }));
  count.textContent = label.text;
  $("chat-search-prev").disabled = label.disabled;
  $("chat-search-next").disabled = label.disabled;
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

async function runChatSearch(query, keepIndex = false) {
  const previous = keepIndex ? chatSearchIndex : -1;
  clearChatSearchMarks();
  const needle = query.trim();
  if (!needle) {
    updateChatSearchCount();
    return;
  }

  // Which bubbles match is decided by wa-ui::search over their rendered text;
  // the text-node ranges are still painted here.
  const bubbles = [...$("messages").querySelectorAll(".bubble[data-id]")];
  const texts = bubbles.map((bubble) =>
    [...bubble.querySelectorAll(".body")]
      .map((body) => body.textContent || "")
      .join("\n"),
  );
  const matches = await invoke("ui_search_matches", { texts, query: needle }).catch(
    () => [],
  );
  const lowered = needle.toLowerCase();
  const ranges = [];
  for (const index of matches) {
    const bubble = bubbles[index];
    if (!bubble) continue;
    for (const body of bubble.querySelectorAll(".body")) {
      const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT);
      let node;
      while ((node = walker.nextNode())) {
        const value = (node.nodeValue || "").toLowerCase();
        let from = 0;
        while (true) {
          const at = value.indexOf(lowered, from);
          if (at < 0) break;
          if (typeof Highlight === "function") {
            const range = document.createRange();
            range.setStart(node, at);
            range.setEnd(node, at + lowered.length);
            ranges.push(range);
          }
          from = at + lowered.length;
        }
      }
    }
    bubble.classList.add("search-hit");
    chatSearchHits.push(bubble);
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

