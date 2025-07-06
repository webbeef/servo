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

function elem(selector) {
  return document.querySelector(selector);
}

document.addEventListener("DOMContentLoaded", () => {
  window["login-button"].onclick = async () => {
    let handle = window.handle.value.trim();
    let password = window.password.value.trim();
    console.log(`Will try login user:${handle} password:${password}`);
    if (handle.length > 0 && password.length > 0) {
      try {
        let session = await navigator.atproto.login(handle, password);
        console.log(`Login successful: ${JSON.stringify(session)}`);
      } catch (e) {
        console.error(e);
      }
      checkCurrentSession();
    }
  };

  window["logout-button"].onclick = async () => {
    try {
      await navigator.atproto.logout();
      console.log(`Logout successful`);
    } catch (e) {
      console.error(e);
    }
    checkCurrentSession();
  };

  window["submit-record"].onclick = async () => {
    let repo = elem("#current-handle").textContent;
    let collection = elem("#record-collection").value;
    let record = JSON.parse(elem("#record-content").value);
    console.log(`Will add ${record} to ${collection} for ${repo}`);

    let object = {
      repo,
      collection,
      record,
    };

    const url = new URL(`at://${repo}/${collection}`);
    const request = new Request(url, {
      method: "POST",
      body: JSON.stringify(object),
      headers: { "Content-Type": "application/json" },
    });
    try {
      let response = await fetch(request);
      elem("#create-record-result").textContent = await response.text();
    } catch (e) {
      console.error(e);
    }
  };

  window["upload-blob"].onclick = async () => {
    let repo = elem("#current-handle").textContent;
    const url = new URL(`at://${repo}`);
    const request = new Request(url, {
      method: "POST",
      body: elem("#blob-content").value,
      headers: { "Content-Type": "text/plain" },
    });
    try {
      let response = await fetch(request);
      elem("#upload-blob-result").textContent = await response.text();
    } catch (e) {
      console.error(e);
    }
  };

  // Perform an initial session check.
  checkCurrentSession();
});

async function checkCurrentSession() {
  try {
    let session = await navigator.atproto.current();
    console.log(JSON.stringify(session));
    document.getElementById("logged-out").classList.add("hidden");
    document.getElementById("logged-in").classList.remove("hidden");
    document.getElementById("current-handle").textContent = session.handle;
  } catch (e) {
    document.getElementById("logged-out").classList.remove("hidden");
    document.getElementById("logged-in").classList.add("hidden");
  }
}
