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

//! Helpers to find a handle's pds.

use std::num::NonZeroUsize;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use atproto_identity::model::Document;
use atproto_identity::resolve::{
    HickoryDnsResolver, InnerIdentityResolver, SharedIdentityResolver, resolve_subject,
};
use atproto_identity::storage::DidDocumentStorage;
use atproto_identity::storage_lru::LruDidDocumentStorage;
use servo_url::ServoUrl;

static PLC_HOSTNAME: &str = "plc.directory";

async fn fetch_did_document(
    document_storage: &LruDidDocumentStorage,
    did: &str,
    dns_resolver: Arc<HickoryDnsResolver>,
) -> Result<Document> {
    if let Ok(Some(document)) = document_storage.get_document_by_did(did).await {
        return Ok(document);
    }

    // Get the document and store it.
    let client_builder = reqwest::Client::builder();
    let http_client = client_builder.build().unwrap();
    let inner_resolver = InnerIdentityResolver {
        dns_resolver,
        http_client,
        plc_hostname: PLC_HOSTNAME.to_owned(),
    };
    let resolver = SharedIdentityResolver(Arc::new(inner_resolver));
    let document = resolver.resolve(did).await?;
    document_storage.store_document(document.clone()).await?;
    Ok(document)
}

pub async fn get_endpoint_for_subject(
    subject: &str,
    document_storage: Option<LruDidDocumentStorage>,
    dns_resolver: Option<Arc<HickoryDnsResolver>>,
) -> Result<(ServoUrl, Document)> {
    let dns_resolver = dns_resolver
        .unwrap_or_else(|| Arc::new(HickoryDnsResolver::create_resolver(Default::default())));
    let document_storage = document_storage
        .unwrap_or_else(|| LruDidDocumentStorage::new(NonZeroUsize::new(1000).unwrap()));

    let client_builder = reqwest::Client::builder();
    let http_client = client_builder.build().unwrap();
    let subject = resolve_subject(&http_client, &*dns_resolver, subject).await?;

    let document = fetch_did_document(&document_storage, &subject, dns_resolver).await?;

    let endpoints = document.pds_endpoints();
    let endpoint = if !endpoints.is_empty() {
        endpoints[0]
    } else {
        return Err(anyhow!("No PDS endpoint"));
    };

    Ok((ServoUrl::parse(endpoint)?, document))
}
