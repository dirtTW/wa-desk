/* WA Desk frontend: emoji. Loaded in order by index.html; all files share the classic-script global scope. */

/* ------------------------------------------------------------------ */
/* wiring                                                              */
/* ------------------------------------------------------------------ */

const EMOJI_RECENT_KEY = "wadesk.emoji.recents";
const EMOJI_CATEGORY_ICONS = ["\u{1F600}", "\u{1F44D}", "\u{1F436}", "\u{1F354}", "\u{2708}\u{FE0F}", "\u{26BD}", "\u{1F4A1}", "\u{2764}\u{FE0F}", "\u{1F3C1}"];

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
  node.innerHTML = recentEmojis()
    .map((char) => `<button type="button" class="emoji" data-char="${char}">${char}</button>`)
    .join("");
}

// "full" emoji support: every Unicode group, searchable, plus recents.
function buildEmojiPicker() {
  if (emojiBuilt) return;
  const groups = window.WA_EMOJI || [];
  const scroll = $("emoji-scroll");
  const cats = $("emoji-cats");

  groups.forEach((group, index) => {
    const title = document.createElement("div");
    title.className = "emoji-title";
    title.dataset.group = String(index);
    title.textContent = group.label;
    scroll.appendChild(title);

    const grid = document.createElement("div");
    grid.className = "emoji-grid";
    grid.dataset.group = String(index);
    for (const [char, keywords] of group.items) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "emoji";
      button.dataset.char = char;
      button.dataset.keywords = keywords;
      button.title = keywords;
      button.textContent = char;
      grid.appendChild(button);
    }
    scroll.appendChild(grid);

    const cat = document.createElement("button");
    cat.type = "button";
    cat.className = index === 0 ? "emoji-cat active" : "emoji-cat";
    cat.dataset.group = String(index);
    cat.title = group.label;
    cat.textContent = EMOJI_CATEGORY_ICONS[index] || "*";
    cats.appendChild(cat);
  });

  renderRecentEmojis();
  emojiBuilt = true;
}

function filterEmoji(query) {
  const scroll = $("emoji-scroll");
  const needle = query.trim().toLowerCase();
  const recents = $("emoji-recents");
  emojiQuery = needle;

  if (!needle) {
    scroll.querySelectorAll(".emoji-title, .emoji-grid").forEach((node) => {
      node.hidden = false;
    });
    recents.hidden = false;
    return;
  }

  recents.hidden = true;
  const matches = [];
  for (const group of window.WA_EMOJI || []) {
    for (const [char, keywords] of group.items) {
      if (keywords.includes(needle)) matches.push([char, keywords]);
    }
  }

  scroll.querySelectorAll(".emoji-title, .emoji-grid").forEach((node) => {
    node.hidden = true;
  });

  let grid = scroll.querySelector(".emoji-grid.search-results");
  if (!grid) {
    grid = document.createElement("div");
    grid.className = "emoji-grid search-results";
    scroll.appendChild(grid);
  }
  grid.hidden = false;
  grid.innerHTML = matches
    .slice(0, 240)
    .map(
      ([char, keywords]) =>
        `<button type="button" class="emoji" data-char="${char}" data-keywords="${keywords}" title="${keywords}">${char}</button>`,
    )
    .join("");
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
  scroll.addEventListener(
    "scroll",
    () => {
      if (emojiQuery) return;
      const titles = [...scroll.querySelectorAll(".emoji-title")];
      const top = scroll.scrollTop + 8;
      let current = 0;
      for (const title of titles) {
        if (title.offsetTop <= top) current = Number(title.dataset.group);
      }
      if (current !== emojiActiveGroup) setActiveEmojiGroup(current);
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

