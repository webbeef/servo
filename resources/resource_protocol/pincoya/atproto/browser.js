/* Copyright (C) 2025 me@webbeef.org
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, version 3.
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * Affero General Public License for more details.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>. */

// Basic at:// data browser.

document.addEventListener("DOMContentLoaded", () => {
  window["start-button"].onclick = () => {
    let handle = window.handle.value.trim();
    console.log(`Handle is ${handle}`);
    if (handle.length > 0) {
      location.hash = `#${handle}`;
    }
  };

  window.onhashchange = (event) => {
    let url = location.hash?.substring(1);
    if (url) {
      atFetch(`at://${url}`);
    }
  };

  window.onhashchange();
});

function router(data) {
  window["result"].innerHTML = "";

  if (data.collections && data.collections.length != 0) {
    return displayRoot(data);
  }

  if (data.records && data.records.length != 0) {
    data.records.forEach(displayRecord);
    return;
  }

  console.log("======= DATA TO ROUTE =======");
  console.log(data);
  console.log("======= DATA TO ROUTE =======");
}

async function atFetch(url) {
  console.log(`Fetching ${url}`);
  try {
    let response = await fetch(url);
    let data = await response.json();
    router(data);
  } catch (e) {
    console.error(e);
  }
}

function currentHandle() {
  let end = location.hash.indexOf("/");
  return location.hash.substring(1, end);
}

function buildImage(record, kind) {
  if (!record.mimeType.startsWith("image/") || !record.ref?.$link) {
    return null;
  }

  let img = document.createElement("img");
  img.classList.add(kind);
  img.src = `at://${currentHandle()}/com.atproto.sync.blob/${record.ref.$link}`;
  return img;
}

function buildSubjectLink(subject) {
  let handle = encodeURIComponent(subject);
  let anchor = document.createElement("a");
  anchor.textContent = subject;
  anchor.onclick = () => {
    location.hash = `#${handle}`;
  };
  return anchor;
}

// Makes sure the authority part is properly encoded.
function sanitizeAtURI(uri) {
  if (!uri.startsWith("at://")) {
    return uri;
  }

  let comps = uri.substring(5).split("/");
  comps[0] = encodeURIComponent(comps[0]);
  return "at://" + comps.join("/");
}

// Displays a single record.
// TODO: instanciate custom elements based on the value's type.
function displayRecord(record) {
  let container = document.createElement("details");
  let summary = document.createElement("summary");
  summary.textContent = record.value.$type;
  let list = document.createElement("ul");

  // Iterate over each property.
  for (let prop in record.value) {
    if (prop == "$type") {
      continue;
    }

    if (prop == "avatar" || prop == "banner") {
      let node = buildImage(record.value[prop], prop);
      if (node) {
        list.append(node);
      }
      continue;
    }

    let value = record.value[prop];
    let item = document.createElement("li");

    if (prop == "subject") {
      let text = document.createTextNode(`${prop}: `);
      item.append(text);
      item.append(buildSubjectLink(value));
    } else {
      item.textContent = `${prop}: ${value}`;
    }
    list.append(item);
  }

  container.append(summary);
  container.append(list);
  let deleteButton = document.createElement("button");
  deleteButton.onclick = async () => {
    try {
      let uri = sanitizeAtURI(record.uri);
      let response = await fetch(uri, { method: "DELETE" });
      console.log(await response.text());
      location.reload();
    } catch (e) {
      console.error(e);
    }
  };
  deleteButton.textContent = "Delete Record";
  container.append(deleteButton);
  window["result"].append(container);
}

// Displays the list of collections attached to a user.
function displayRoot(data) {
  let result = window["result"];
  let list = document.createElement("ul");
  data.collections.forEach((collection) => {
    let item = document.createElement("li");
    let anchor = document.createElement("a");
    anchor.setAttribute("href", `#${data.handle}/${collection}`);
    anchor.textContent = collection;
    item.append(anchor);
    list.append(item);
  });
  result.append(list);
}
