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

use serde::Deserialize;
use servo_url::ServoUrl;

#[derive(Debug, Deserialize)]
pub(crate) struct ZoneFile {
    zones: Vec<Zone>,
}

impl ZoneFile {
    pub fn find_zone(&self, name: &str) -> Option<&Zone> {
        self.zones.iter().find(|zone| zone.name == name)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct Zone {
    name: String,
    mappings: Vec<ZoneMapping>,
}

impl Zone {
    pub fn find_mapping(&self, path: &str) -> Option<&ZoneMapping> {
        self.mappings.iter().find(|mapping| mapping.path == path)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ZoneMapping {
    path: String,
    pub source: String,
}

pub(crate) trait TrustedZoneUrl {
    fn zone_and_user(&self) -> Option<(String, String)>;
}

impl TrustedZoneUrl for ServoUrl {
    fn zone_and_user(&self) -> Option<(String, String)> {
        let domain = self.domain()?;
        let mut parts = domain.splitn(2, '.');
        let zone = parts.next().map(|z| z.to_owned())?;
        let user = parts.next().map(|u| u.to_owned())?;
        Some((zone, user))
    }
}
