/* WA Desk frontend: emoji. Loaded in order by index.html; all files share the classic-script global scope. */

/* ------------------------------------------------------------------ */
/* wiring                                                              */
/* ------------------------------------------------------------------ */

const EMOJI_RECENT_KEY = "wadesk.emoji.recents";

let emojiBuilt = false;
let emojiQuery = "";
let emojiActiveGroup = 0;

function updateSendState() {
  $("send").disabled = $("input").value.trim().length === 0;
}

function recentEmojis() {
  try {
    const parsed = JSON.parse(localStorage.getItem(EMOJI_RECENT_KEY) || "[]");
    return Array.isArray(parsed) ? parsed.filter((value) => typeof value === "string") : [];
  } catch {
    return [];
  }
}

function rememberEmoji(char) {
  const recents = recentEmojis().filter((value) => value !== char);
  recents.unshift(char);
  try {
    localStorage.setItem(EMOJI_RECENT_KEY, JSON.stringify(recents.slice(0, 24)));
  } catch {
    /* storage full: recents are a nicety */
  }
  renderRecentEmojis();
}

function renderRecentEmojis() {
  const node = $("emoji-recents");
  if (!node) return;
  // The row markup comes from wa-ui::emoji; the list itself is local storage.
  invoke("ui_emoji_recents", { chars: recentEmojis() })
    .then((html) => {
      node.innerHTML = html;
    })
    .catch(() => {});
}

// "full" emoji support: every Unicode group, searchable, plus recents.
//
// The catalogue and the picker markup live in Rust (`wa-ui`): the webview only
// injects the HTML fetched at boot and asks Rust for search results.
function buildEmojiPicker() {
  if (emojiBuilt) return;
  const scroll = $("emoji-scroll");
  const cats = $("emoji-cats");
  scroll.innerHTML = uiAssets.emojiPicker || "";
  cats.innerHTML = uiAssets.emojiCategories || "";
  renderRecentEmojis();
  emojiBuilt = true;
}

async function filterEmoji(query) {
  const scroll = $("emoji-scroll");
  const needle = query.trim();
  emojiQuery = needle.toLowerCase();
  const recents = $("emoji-recents");

  if (!needle) {
    scroll.querySelectorAll(".emoji-title, .emoji-grid").forEach((node) => {
      node.hidden = false;
    });
    const stale = scroll.querySelector(".emoji-grid.search-results");
    if (stale) stale.remove();
    recents.hidden = false;
    return;
  }

  recents.hidden = true;
  scroll.querySelectorAll(".emoji-title, .emoji-grid").forEach((node) => {
    node.hidden = true;
  });
  const html = await invoke("ui_emoji_search", { query: needle, limit: 240 });
  if (emojiQuery !== needle.toLowerCase()) return;
  let grid = scroll.querySelector(".emoji-grid.search-results");
  if (!grid) {
    grid = document.createElement("div");
    grid.className = "emoji-grid search-results";
    scroll.appendChild(grid);
  }
  grid.hidden = false;
  grid.innerHTML = html.replace(/^<div[^>]*>|<\/div>$/g, "");
}

function scrollToEmojiGroup(index) {
  const title = $("emoji-scroll").querySelector(`.emoji-title[data-group="${index}"]`);
  if (title) title.scrollIntoView({ block: "start" });
  setActiveEmojiGroup(index);
}

function setActiveEmojiGroup(index) {
  emojiActiveGroup = index;
  $("emoji-cats")
    .querySelectorAll(".emoji-cat")
    .forEach((node) => node.classList.toggle("active", node.dataset.group === String(index)));
}

function bindEmojiScroll() {
  const scroll = $("emoji-scroll");
  let groupScrollQueued = false;
  scroll.addEventListener(
    "scroll",
    () => {
      if (emojiQuery || groupScrollQueued) return;
      // The category maths is wa_ui::emoji::active_group; ask once per frame
      // instead of per scroll event.
      groupScrollQueued = true;
      // setTimeout rather than requestAnimationFrame: rAF is suspended while the
      // window is hidden/occluded, which would leave the flag stuck on.
      setTimeout(async () => {
        groupScrollQueued = false;
        if (emojiQuery) return;
        const titles = [...scroll.querySelectorAll(".emoji-title")];
        const current = await invoke("ui_emoji_active_group", {
          offsets: titles.map((title) => [Number(title.dataset.group), title.offsetTop]),
          scrollTop: scroll.scrollTop,
        }).catch(() => emojiActiveGroup);
        if (current !== emojiActiveGroup) setActiveEmojiGroup(current);
      }, 50);
    },
    { passive: true },
  );
}

let reactionPickMessage = null;

function toggleEmoji(force) {
  const panel = $("emoji-panel");
  const show = force ?? panel.hidden;
  panel.hidden = !show;
  if (!show) {
    reactionPickMessage = null;
    return;
  }

  buildEmojiPicker();
  if (!panel.dataset.bound) {
    panel.dataset.bound = "1";
    bindEmojiScroll();
  }
  $("emoji-scroll").scrollTop = 0;
  if (currentPickerTab() === "stickers") reloadStickerView();
  else if (currentPickerTab() === "emoji") $("emoji-search").focus();
}

function currentPickerTab() {
  const active = $("emoji-panel").querySelector(".picker-tab.active");
  return active ? active.dataset.tab : "emoji";
}

function setPickerTab(tab) {
  $("emoji-panel")
    .querySelectorAll(".picker-tab")
    .forEach((node) => node.classList.toggle("active", node.dataset.tab === tab));
  $("emoji-view").hidden = tab !== "emoji";
  $("sticker-view").hidden = tab !== "stickers";
  $("gif-view").hidden = tab !== "gif";
  if (tab === "stickers") reloadStickerView();
  else if (tab === "emoji") $("emoji-search").focus();
}

function insertEmoji(emoji) {
  const input = $("input");
  const start = input.selectionStart ?? input.value.length;
  const end = input.selectionEnd ?? start;
  input.value = input.value.slice(0, start) + emoji + input.value.slice(end);
  const caret = start + emoji.length;
  input.focus();
  input.setSelectionRange(caret, caret);
  autosize(input);
  updateSendState();
  startTyping();
  rememberEmoji(emoji);
}

