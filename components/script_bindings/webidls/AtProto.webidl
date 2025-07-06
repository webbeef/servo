/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// TODO: expose some methods only to internal pages, and figure out the privacy story.

dictionary AtProtoSession {
    required USVString did;
    required USVString handle;
};

[Exposed=Window,
Func="AtProto::is_pincoya_api"]
interface AtProto {
    // Tries to login with the submitted credentials.
    // Resolves with the new session if successful, rejects otherwise.
    Promise<AtProtoSession> login(USVString handle, USVString password);

    // Revokes the current session if it exists.
    Promise<undefined> logout();

    // Resolves with the logged in user DID if any, rejects otherwise.
    Promise<AtProtoSession> current();
};

partial interface Navigator {
    [Func="AtProto::is_pincoya_api"]
    readonly attribute AtProto atproto;
};
