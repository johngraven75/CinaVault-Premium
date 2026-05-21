pub const PHOENIX_ADULT_PROVIDER_GUID: &str = "8f97371f-8617-463c-9859-a33072182494";
pub const PHOENIX_ADULT_PLUGIN_UNIQUE_ID: &str = "dc40637f-6ebd-4a34-b8a1-8799629120cf";
pub const PHOENIX_ADULT_NAME: &str = "PhoenixAdult";
pub const PHOENIX_ADULT_OWNER: &str = "DirtyRacer";
pub const PHOENIX_ADULT_VERSION: &str = "2.7.0.47";
pub const PHOENIX_ADULT_TARGET_ABI: &str = "10.8.0.0";
pub const PHOENIX_ADULT_CHECKSUM: &str = "e0c4290082855002f02b957024a90200";
pub const PHOENIX_ADULT_SOURCE_URL: &str = "https://github.com/DirtyRacer1337/Jellyfin.Plugin.PhoenixAdult/releases/download/2.7.0.47/Jellyfin.Plugin.PhoenixAdult.zip";
pub const PHOENIX_ADULT_DESCRIPTION: &str =
    "This metadata provider helps fill Jellyfin/Emby with information for adult videos by pulling from the original site.";
pub const PHOENIX_ADULT_OVERVIEW: &str = "Manage Your Adult Videos in Jellyfin/Emby";

pub fn phoenix_adult_default_configuration() -> serde_json::Value {
    serde_json::json!({
        "FlareSolverrURL": "",
        "DisableCaching": false,
        "DisableSSLCheck": false,
        "ProxyEnable": false,
        "ProxyHost": "",
        "ProxyPort": 0,
        "ProxyLogin": "",
        "ProxyPassword": "",
        "UseFilePath": false,
        "DefaultSiteName": "",
        "UseMetadataAPI": false,
        "MetadataAPIToken": "",
        "DisableActors": false,
        "DisableGenres": false,
        "DisableImageValidation": false,
        "DisableImageSize": false,
        "DisableAutoIdentify": false,
        "PreferedActorNameSource": "LocalDatabase",
        "JAVActorNamingStyle": "WesternStyle",
        "GenresSortingStyle": "Alphabetical"
    })
}

pub fn phoenix_adult_configuration_schema() -> serde_json::Value {
    serde_json::json!({
        "checkboxes": [
            "DisableCaching",
            "DisableSSLCheck",
            "ProxyEnable",
            "UseFilePath",
            "UseMetadataAPI",
            "DisableActors",
            "DisableGenres",
            "DisableImageValidation",
            "DisableImageSize",
            "DisableAutoIdentify"
        ],
        "secrets": [
            "ProxyPassword",
            "MetadataAPIToken"
        ],
        "selects": {
            "PreferedActorNameSource": ["LocalDatabase", "NoChange"],
            "JAVActorNamingStyle": ["WesternStyle", "JapaneseStyle"],
            "GenresSortingStyle": ["Alphabetical", "PositionsLast"]
        }
    })
}

pub fn phoenix_adult_manifest_summary() -> serde_json::Value {
    serde_json::json!({
        "category": "Metadata",
        "guid": PHOENIX_ADULT_PROVIDER_GUID,
        "pluginUniqueId": PHOENIX_ADULT_PLUGIN_UNIQUE_ID,
        "name": PHOENIX_ADULT_NAME,
        "description": PHOENIX_ADULT_DESCRIPTION,
        "owner": PHOENIX_ADULT_OWNER,
        "overview": PHOENIX_ADULT_OVERVIEW,
        "configuration": phoenix_adult_default_configuration(),
        "configuration_schema": phoenix_adult_configuration_schema(),
        "latest": {
            "checksum": PHOENIX_ADULT_CHECKSUM,
            "changelog": "Added Latest Jellyfin Support (10.8.0); Added Latest Emby Support (4.7); Minor Changes",
            "targetAbi": PHOENIX_ADULT_TARGET_ABI,
            "sourceUrl": PHOENIX_ADULT_SOURCE_URL,
            "timestamp": "2022-06-21T21:24:12Z",
            "version": PHOENIX_ADULT_VERSION
        }
    })
}
