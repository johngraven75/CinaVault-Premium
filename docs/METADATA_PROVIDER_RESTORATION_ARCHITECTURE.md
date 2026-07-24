# CinaVault Metadata Provider Restoration Architecture

## Overview

The CinaVault Metadata Provider Restoration Architecture is a comprehensive system for discovering, routing, and processing metadata requests across multiple providers, with specialized support for adult content providers (PGMA Modernized and Porn Site Nuxt). The architecture implements a layered approach with provider normalization, fallback logic, and enhancement bridges for complete metadata enrichment.

## Architecture Components

### 1. Command Routing & Provider Discovery

**Entry Point:** `main.rs:150`

The Tauri command system routes all metadata operations through the `metadata_ext` module, which serves as the central dispatcher for provider discovery and normalization.

#### Provider Discovery Flow

```
Tauri Command System
├── main.rs command registration (main.rs:150)
│   └── metadata_ext::get_metadata_providers
└── metadata_ext.rs provider discovery (metadata_ext.rs:167)
    ├── crate::metadata::get_metadata_providers() (metadata_ext.rs:168)
    │   └── Loads 30+ standard providers
    ├── PGMA provider injection (metadata_ext.rs:173)
    │   └── Adds "PGMA Modernized" to list
    └── Porn Site Nuxt injection (metadata_ext.rs:185)
        └── Adds adult content provider
```

#### Key Functions

- **`get_metadata_providers()`** (metadata_ext.rs:167): Returns comprehensive provider list by merging legacy providers with specialized adult content providers
- **Provider Injection Logic**: Ensures PGMA and Porn Site Nuxt providers are always available in the provider catalog

### 2. Provider Normalization System

**Location:** `metadata_ext.rs:22-35`

The normalization system handles provider key aliases and ensures consistent routing regardless of user input format.

#### Normalization Rules

```rust
fn normalize_provider_key(provider: &str) -> String {
    // PGMA aliases: "pgma", "pgma-modernized", "pgma_modernized", "pgma modernized", "plex pgma"
    // Porn Site Nuxt aliases: handled via is_porn_site_nuxt_alias()
    // Standard providers: tmdb, tpdb, omdb with various aliases
}
```

#### Supported Aliases

- **PGMA**: `pgma`, `pgma-modernized`, `pgma_modernized`, `pgma modernized`, `plex pgma`
- **TMDb**: `themoviedb`, `themoviedb_images`, `tmdb_images`, `tmdb`
- **TPDb**: `theporndb`, `tpdb`
- **OMDb**: `open_movie_db`, `openmoviedb`, `omdb`

### 3. Adult Content Provider Processing

**Entry Point:** `metadata_ext.rs:197`

Specialized routing for adult content providers with fallback logic and HTTP-based metadata fetching.

#### Processing Flow

```
fetch_metadata() command entry (metadata_ext.rs:197)
├── normalize_provider_key() (metadata_ext.rs:202)
├── match provider.as_str() routing (metadata_ext.rs:203)
│   ├── PGMA_PROVIDER_KEY branch (metadata_ext.rs:204)
│   │   └── pgma_metadata_response() - Local bridge implementation
│   ├── "porn_site_nuxt" branch (metadata_ext.rs:205)
│   │   ├── build HTTP client (metadata_ext.rs:206)
│   │   └── fetch_porn_site_nuxt_results() (metadata_ext.rs:210)
│   └── fallback branch (metadata_ext.rs:212)
│       └── crate::metadata::fetch_metadata() - Legacy providers
└── return Result<serde_json::Value>
```

#### PGMA Local Bridge

**Location:** `metadata_ext.rs:61-85`

The PGMA provider implements a local sidecar bridge that:

- Cleans local metadata titles by removing resolution/codec noise
- Returns structured JSON with native bridge indicators
- Provides capability information (local_sidecar_nfo, poster_folder_cover_artwork, etc.)

#### Porn Site Nuxt HTTP Client

**Location:** `metadata_ext.rs:107-144`

Features:
- 4-second timeout for search requests
- Configurable base URL support
- JSON response parsing with entry transformation
- Error handling with HTTP status code reporting

### 4. Metadata Check & Database Update Flow

**Entry Point:** `metadata_ext.rs:228`

Individual media item metadata checking with legacy fallback and adult provider specialization.

#### Check Flow

```
check_media_item_metadata() entry (metadata_ext.rs:228)
├── Legacy metadata check attempt (metadata_ext.rs:232)
│   └── Early return if match found (metadata_ext.rs:233)
└── Adult provider fallback path (metadata_ext.rs:276)
    ├── Query Porn Site Nuxt API (metadata_ext.rs:281)
    └── Update database with results (metadata_ext.rs:306)
```

#### Database Update Operations

**Location:** `metadata_ext.rs:306-318`

Updates the following fields:
- `title`: Matched title from provider
- `overview`: Content description
- `poster_path`: Poster image URL
- `rating`: Numeric rating value
- `genre`: Content genre (set to "Adult")
- `media_type`: Set to "adult" for adult content

#### Field Change Tracking

The system tracks which fields were updated:
```rust
let changed_fields = usize::from(matched_title.as_deref() != Some(title.as_str()))
    + usize::from(overview.is_some())
    + usize::from(poster_path.is_some())
    + usize::from(rating.is_some())
    + usize::from(genre.is_some())
    + usize::from(!media_type.eq_ignore_ascii_case("adult"));
```

### 5. Enhancement Bridge & Backdrop Fetching

**Entry Point:** `metadata_bridge.rs:78`

The metadata bridge enhances base metadata with additional artwork and complete item snapshots.

#### Bridge Flow

```
Metadata Enhancement Bridge System
├── check_media_item_metadata() (metadata_bridge.rs:78)
│   ├── Calls metadata_ext for base check (metadata_bridge.rs:78)
│   ├── Queries DB for backdrop request (metadata_bridge.rs:83)
│   │   └── SELECT tmdb_id, media_type, backdrop_path, api_key
│   ├── fetch_tmdb_backdrop() if needed (metadata_bridge.rs:108)
│   │   ├── Builds HTTP client (metadata_bridge.rs:6)
│   │   ├── Tries TV endpoint first (metadata_bridge.rs:10)
│   │   └── Falls back to Movie endpoint (metadata_bridge.rs:10)
│   ├── UPDATE media_items SET backdrop_path (metadata_bridge.rs:111)
│   └── full_media_item() for snapshot (metadata_bridge.rs:120)
│       └── SELECT all media fields (metadata_bridge.rs:39)
└── Response enhancement
    ├── result["updated_item"] = item (metadata_bridge.rs:122)
    └── Merges all item fields into result (metadata_bridge.rs:124)
```

#### Backdrop Fetching Strategy

**Location:** `metadata_bridge.rs:5-33`

- **Preferred Endpoint Order**: TV → Movie for tvshow/episode/tv, Movie → TV for others
- **Timeout**: 10 seconds per request
- **URL Format**: `https://api.themoviedb.org/3/{endpoint}/{tmdb_id}?api_key={api_key}`
- **Image URL**: `https://image.tmdb.org/t/p/w1280{backdrop_path}`

#### Complete Item Snapshot

**Location:** `metadata_bridge.rs:35-72`

Returns comprehensive media item data including:
- Basic metadata (id, title, file_path, media_type)
- Technical details (year, rating, duration, file_size, resolution, codec)
- Artwork (poster_path, backdrop_path)
- User data (verified, watched, favorite, date_added, last_played)
- External IDs (tmdb_id, imdb_id, source_id)

## API Key Management

### Provider Status

**Location:** `metadata_ext.rs:388-413`

- Queries configured providers from `api_keys` table
- Returns normalized provider keys with configuration status
- Includes total provider count

### API Key Operations

- **`set_api_key()`** (metadata_ext.rs:445): Insert or replace provider API keys
- **`get_api_keys()`** (metadata_ext.rs:462): Retrieve masked API keys (shows first 2 and last 2 characters)
- **`test_api_key()`** (metadata_ext.rs:416): Validate API keys with provider-specific testing

### Key Masking

```rust
let masked = if key.len() > 4 {
    format!("{}...{}", &key[..2], &key[key.len() - 2..])
} else {
    "****".to_string()
};
```

## Error Handling

### Provider Error Collection

**Location:** `metadata_ext.rs:155-164`

Errors are accumulated in a `provider_errors` array in the response JSON, allowing multiple provider failures to be reported without stopping the overall operation.

### Error Propagation

- HTTP errors include status codes (e.g., "Porn Site Nuxt provider returned http_404")
- Database errors are converted to string messages
- Timeout errors are caught and reported
- Legacy provider errors are preserved in fallback scenarios

## Testing

### Unit Tests

**Location:** `metadata_ext.rs:489-522`

1. **Provider Alias Testing**: Verifies normalization of PGMA and Porn Site Nuxt aliases
2. **Provider Catalog Testing**: Ensures PGMA and Porn Site Nuxt are included in provider list
3. **PGMA Bridge Testing**: Validates local bridge metadata response structure

### Bridge Contract Tests

**Location:** `metadata_bridge.rs:132-144`

Ensures the `updated_item` envelope is maintained in bridge responses with required fields (id, poster_path, backdrop_path).

## Configuration

### Provider Constants

```rust
const PGMA_PROVIDER_KEY: &str = "pgma";
const PGMA_PROVIDER_BASE_URL: &str = "cinavault://pgma-bridge";
const PORN_SITE_NUXT_DEFAULT_BASE_URL: &str = // from adult_site_provider
```

### Timeout Configuration

- **Porn Site Nuxt Search**: 4 seconds
- **Porn Site Nuxt Fetch**: 8 seconds  
- **TMDb Backdrop**: 10 seconds

## Data Flow Summary

1. **Discovery**: `get_metadata_providers()` merges legacy + adult providers
2. **Normalization**: `normalize_provider_key()` standardizes provider keys
3. **Routing**: `fetch_metadata()` dispatches to appropriate provider handler
4. **Processing**: Provider-specific logic (local bridge or HTTP request)
5. **Fallback**: Legacy providers handle unconfigured adult requests
6. **Enhancement**: `metadata_bridge` adds backdrops and complete snapshots
7. **Persistence**: Database updates with normalized metadata fields

## Integration Points

### Tauri Commands

Registered in `main.rs:143-150`:
- `fetch_metadata`
- `search_metadata`
- `check_media_item_metadata`
- `get_provider_status`
- `test_api_key`
- `set_api_key`
- `get_api_keys`
- `get_metadata_providers`

### Database Schema

Required tables:
- `media_items`: Stores metadata and file information
- `api_keys`: Stores provider API configurations

### External Dependencies

- **reqwest**: HTTP client for API requests
- **rusqlite**: Database operations
- **serde_json**: JSON serialization/deserialization

## Performance Considerations

- **Early Return**: Legacy metadata check returns immediately on success
- **Timeout Protection**: All HTTP requests have configured timeouts
- **Lazy Loading**: Backdrop fetching only occurs when missing
- **Batch Operations**: Database updates use parameterized queries
- **Error Accumulation**: Multiple provider errors collected without early termination

## Security Considerations

- **API Key Masking**: Keys are partially masked in UI responses
- **SQL Injection Prevention**: Parameterized queries used throughout
- **Input Validation**: Provider keys normalized and validated before routing
- **Timeout Protection**: Prevents hanging on unresponsive external APIs

## Future Enhancement Opportunities

1. **Caching Layer**: Add response caching for frequently accessed metadata
2. **Parallel Provider Queries**: Query multiple providers simultaneously
3. **Backoff Strategy**: Implement exponential backoff for failed requests
4. **Provider Health Monitoring**: Track provider availability and response times
5. **Metadata Versioning**: Track metadata update history for rollback capability
