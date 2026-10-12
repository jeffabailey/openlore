(function () {
  'use strict';
  var input = document.getElementById('handle');
  var list = document.getElementById('handle-suggestions');
  if (!input || !list || !window.fetch) return;
  var AVATAR = 'https://cdn.bsky.app/img/avatar/plain/';
  var THUMBNAIL = 'https://cdn.bsky.app/img/avatar_thumbnail/plain/';
  var timer = null, latest = '', actors = [], active = -1;

  function close() {
    list.hidden = true;
    input.setAttribute('aria-expanded', 'false');
    input.removeAttribute('aria-activedescendant');
    active = -1;
  }

  function highlight(i) {
    var items = list.children;
    if (items[active]) items[active].setAttribute('aria-selected', 'false');
    active = i;
    if (!items[i]) { input.removeAttribute('aria-activedescendant'); return; }
    items[i].setAttribute('aria-selected', 'true');
    input.setAttribute('aria-activedescendant', items[i].id);
    items[i].scrollIntoView({ block: 'nearest' });
  }

  function choose(i) {
    if (!actors[i]) return;
    input.value = actors[i].handle;
    latest = input.value;
    close();
  }

  function span(className, text) {
    var s = document.createElement('span');
    s.className = className;
    s.textContent = text;
    return s;
  }

  function avatar(a) {
    if (typeof a.avatar === 'string' && a.avatar.indexOf(AVATAR) === 0) {
      var img = document.createElement('img');
      img.className = 'typeahead-avatar';
      img.alt = '';
      img.width = 32;
      img.height = 32;
      img.decoding = 'async';
      img.referrerPolicy = 'no-referrer';
      img.src = THUMBNAIL + a.avatar.slice(AVATAR.length);
      return img;
    }
    var initial = span('typeahead-avatar', (a.displayName || a.handle).charAt(0).toUpperCase());
    initial.setAttribute('aria-hidden', 'true');
    return initial;
  }

  function option(a, i) {
    var li = document.createElement('li');
    li.id = 'handle-option-' + i;
    li.className = 'typeahead-option';
    li.setAttribute('role', 'option');
    li.setAttribute('aria-selected', 'false');
    var text = span('typeahead-text', '');
    text.append(span('typeahead-name', a.displayName || a.handle), span('typeahead-handle', '@' + a.handle));
    li.append(avatar(a), text);
    li.addEventListener('mousedown', function (event) { event.preventDefault(); choose(i); });
    return li;
  }

  function show(found) {
    actors = found;
    list.replaceChildren.apply(list, found.map(option));
    active = -1;
    if (!found.length) { close(); return; }
    list.hidden = false;
    input.setAttribute('aria-expanded', 'true');
  }

  input.addEventListener('input', function () {
    var q = input.value.trim().replace(/^@/, '');
    clearTimeout(timer);
    latest = q;
    if (q.length < 2) { show([]); return; }
    timer = setTimeout(function () {
      fetch('https://public.api.bsky.app/xrpc/app.bsky.actor.searchActorsTypeahead?limit=8&q=' + encodeURIComponent(q))
        .then(function (r) { return r.ok ? r.json() : { actors: [] }; })
        .then(function (body) {
          if (q !== latest) return;
          show((body.actors || []).filter(function (a) { return typeof a.handle === 'string'; }));
        })
        .catch(function () {});
    }, 200);
  });

  input.addEventListener('keydown', function (event) {
    if (list.hidden || !actors.length) return;
    if (event.key === 'ArrowDown') { event.preventDefault(); highlight((active + 1) % actors.length); }
    else if (event.key === 'ArrowUp') { event.preventDefault(); highlight(active <= 0 ? actors.length - 1 : active - 1); }
    else if (event.key === 'Enter' && active >= 0) { event.preventDefault(); choose(active); }
    else if (event.key === 'Escape') { event.preventDefault(); close(); }
  });

  input.addEventListener('blur', close);
})();
