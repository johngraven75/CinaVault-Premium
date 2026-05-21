pub const THEPORNDB_PROVIDER_NAME: &str = "ThePornDB";
pub const THEPORNDB_PROVIDER_KEY: &str = "tpdb";
pub const THEPORNDB_API_BASE_URL: &str = "https://api.theporndb.net";

pub fn theporndb_scene_search_url(query: &str) -> String {
    let encoded = percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);
    format!("{THEPORNDB_API_BASE_URL}/scenes?parse={encoded}&hash=&year=")
}

pub fn theporndb_default_configuration() -> serde_json::Value {
    serde_json::json!({
        "MetadataAPIToken": "",
        "UseFilePath": false,
        "UseOSHash": false,
        "OrderStyle": "Default",
        "TagStyle": "Genre",
        "AddCollectionOnSite": false,
        "CollectionMinSize": 0,
        "AddCollectionToCollections": true,
        "CollectionType": "All",
        "StudioStyle": "All",
        "UseCustomTitle": false,
        "CustomTitle": "{studio}: {title} ({actors})",
        "UseUnmatchedTag": false,
        "UnmatchedTag": "Missing From ThePornDB",
        "DisableMediaAutoIdentify": false,
        "DisableActorsAutoIdentify": false,
        "DisableMaleActors": false,
        "DisableActors": false,
        "DisableDirectors": false,
        "DisableGenres": false,
        "ScenesImage": "Poster",
        "AddDisambiguation": true,
        "ActorsRole": "Gender",
        "ActorsImage": "Poster",
        "ActorsOverview": "Default",
        "ActorsOverviewFormat": "<strong style=\"color:#ff0000\">{measurements}<br/></strong>{cupsize}-{waist}-{hips}<br/>{tattoos}<br/>{piercings}<br/>{bio}"
    })
}

pub fn theporndb_configuration_schema() -> serde_json::Value {
    serde_json::json!({
        "OrderStyle": { "Default": 0, "DistanceByTitle": 1 },
        "TagStyle": { "Genre": 0, "Tag": 1, "Disabled": 2 },
        "CollectionType": { "Scene": 0, "Movie": 1, "JAV": 2, "All": 3 },
        "StudioStyle": { "Site": 0, "Network": 1, "All": 2, "Parent": 3 },
        "ScenesImageStyle": { "Poster": 0, "Background": 1 },
        "ActorsOverviewStyle": { "None": 0, "Default": 1, "CustomExtras": 2 },
        "ActorsRoleStyle": { "None": 0, "Gender": 1, "NameByScene": 2 },
        "ActorsImageStyle": { "Poster": 0, "Face": 1 }
    })
}

pub fn theporndb_provider_manifest_summary() -> serde_json::Value {
    serde_json::json!({
        "category": "Metadata",
        "name": THEPORNDB_PROVIDER_NAME,
        "key": THEPORNDB_PROVIDER_KEY,
        "base_url": THEPORNDB_API_BASE_URL,
        "configuration": theporndb_default_configuration(),
        "configuration_schema": theporndb_configuration_schema()
    })
}
