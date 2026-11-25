# Operations

## Interactive flows

- [OAuth authorization](../.archify/workflow-berry-operations-20261120-173118/autorizacao.html)
- [Video publication](../.archify/workflow-berry-operations-20261120-173118/publicacao.html)

Open the downloaded HTML locally. Diagrams are based on an isolated snapshot of the local source, with a placeholder repository identity because this project does not yet have a Git history or remote. The diagram labels are in Portuguese; viewer controls fall back to English.

## Tools and inputs

All tool names are the exact MCP names. Optional values can be omitted; paths supplied to upload tools must be absolute and readable by the server process.

### `canal`

No arguments. Returns the authenticated channel and its uploads playlist information where available.

### `consultar_video`

```json
{"video_id":"VIDEO_ID_11"}
```

Queries video details, including `status` and `processingDetails` when returned. Nonexistent or inaccessible IDs produce an error.

### `listar_uploads`

```json
{"max_results":20}
```

Optionally send `page_token` from the prior response's `nextPageToken`. A page has at most 50 items. Each item contains the playlist entry and its video details, or `video: null` when details were unavailable. Checking one page cannot guarantee that an upload is not duplicated.

### `publicar_video`

```json
{
  "file_path":"/absolute/path/video.mp4",
  "title":"Example video",
  "description":"Description",
  "made_for_kids":false,
  "privacy":"private",
  "publish_at":"2027-01-15T19:00:00-03:00",
  "category_id":"10",
  "tags":["music"],
  "contains_synthetic_media":true,
  "captions":[{"file_path":"/absolute/path/subtitles.srt","language":"pt-BR","name":"Português","is_draft":false}]
}
```

Only `file_path`, `title`, and `made_for_kids` are required. Allowed privacy values: `public`, `private`, `unlisted`. Without a schedule, omitted privacy defaults to public. `publish_at` requires private privacy and uses RFC 3339 with an offset. The category defaults to `10`. Omitting `contains_synthetic_media` sends no declaration; `false` sends an explicit negative declaration.

The optional `captions` array attaches timestamped subtitle files (for example SRT or VTT, up to 100 MB in total per upload request) after the video is uploaded. Each track requires an absolute `file_path`, `language`, and `name`; `is_draft` defaults to false. Caption files are read before video creation. The successful video response includes `mcp_legendas` with each track's result: if a track fails, the video already exists, so retain its ID and use `anexar_legenda` rather than uploading the video again. YouTube may not have finished processing the video immediately. `description` is the video description, not a subtitle track.

The upload begins a resumable session and streams the file. Automatic resume after interruption is not implemented. If the outcome is uncertain, inspect the channel before retrying. YouTube may restrict privacy for uploads from unaudited API projects. YouTube decides whether a video is classified as a Short.

### `declarar_conteudo_sintetico`

```json
{"video_id":"VIDEO_ID_11","contains_synthetic_media":true}
```

Reads the current status, preserves its writable fields and sends an ETag precondition with the update. Missing necessary fields, a missing ETag, or a scheduled publication less than one minute away stops the update. Verify the result and scheduling after a successful request; do not infer a negative declaration from an absent response field.

### `definir_miniatura`

```json
{
  "video_id":"VIDEO_ID_11",
  "file_path":"/absolute/path/thumbnail.png"
}
```

Uploads a custom thumbnail for a video accessible to the authorized channel. The path must be absolute; the image must be JPEG or PNG and no larger than 50 MB. The API response confirms the uploaded thumbnail resources. YouTube may take a short time to propagate the image to all surfaces. For Shorts, set the thumbnail after video processing has succeeded.

## Captions

### `anexar_legenda`

```json
{"video_id":"VIDEO_ID_11","caption":{"file_path":"/absolute/path/subtitles.vtt","language":"en","name":"English","is_draft":false}}
```

Uploads a timestamped subtitle track to an existing video. Files must be readable by the MCP server process. YouTube charges quota for each caption upload; check existing tracks before retrying an uncertain upload.

## Statistics and analytics

### `obter_estatisticas`

```json
{"resource":"video","ids":["VIDEO_ID_11"]}
```

`resource` may be `video`, `channel`, or `playlist`; provide 1–50 IDs, except that `channel` may omit `ids` to request the authorized channel. The response is the unchanged Data API JSON. Video and channel results include their available `statistics`; playlists expose `contentDetails.itemCount` rather than a Data API statistics resource. Counts reflect what the API and account permissions make available.

### `consultar_analytics`

```json
{"start_date":"2026-01-01","end_date":"2026-01-31","metrics":"views,estimatedMinutesWatched","dimensions":"day","filters":"video==VIDEO_ID_11","sort":"day","max_results":50,"start_index":1}
```

Returns the complete unmodified Analytics API JSON (`columnHeaders` and `rows`), including an empty report when supplied by Google. Dates are inclusive `YYYY-MM-DD`. Optional fields include `ids` (defaults to `channel==MINE`), `dimensions`, `filters`, `sort`, `currency`, `max_results`, `start_index`, and `include_historical_channel_data`. Any API-supported metric and dimension can be requested; the [channel report catalogue](https://developers.google.com/youtube/analytics/channel_reports) defines valid combinations. Use `filters` for a particular video or playlist when the requested report supports it. Availability depends on channel permissions, data processing and minimum thresholds; monetary metrics require access and the monetary OAuth scope. These are API-provided aggregated measurements, not raw individual viewing events, and the API does not expose every Studio metric.

## Bulk reporting

Enable the YouTube Reporting API. These tools return Google's unmodified JSON, except `baixar_relatorio`, which streams the original CSV to a new local file and returns its path, size, and report metadata. Available report types and data depend on the account and report permissions.

1. `listar_tipos_relatorios` with `{}` discovers report type IDs; `listar_jobs_relatorios` lists existing jobs. Both accept `page_size`, `page_token`, `include_system_managed`, and `on_behalf_of_content_owner`.
2. `criar_job_relatorio` with `{"report_type_id":"channel_basic_a3","name":"Daily channel activity"}` requests daily generation. System-managed reports are created by YouTube rather than through this tool. Check existing jobs before creating another. New reports are not immediately available and can take up to 48 hours.
3. `listar_relatorios` with `{"job_id":"JOB_ID"}` lists generated reports. Optional `page_size`, `page_token`, `created_after`, `start_time_at_or_after`, `start_time_before`, and `on_behalf_of_content_owner` follow the Reporting API. Timestamp filters use RFC 3339 UTC.
4. `baixar_relatorio` with `{"job_id":"JOB_ID","report_id":"REPORT_ID","file_path":"/absolute/path/report.csv"}` downloads one report. `on_behalf_of_content_owner` is optional. Existing files are never overwritten; failed downloads remove the partial file.

## Comments and share links

### `listar_comentarios`, `listar_respostas`, `responder_comentario`

```json
{"video_id":"VIDEO_ID_11","max_results":100,"order":"time","include_replies":true}
```

This returns the raw comment-thread page. Set optional `page_token` to `nextPageToken`; `order` accepts `time` or `relevance`. Embedded replies may be incomplete: call `listar_respostas` with `{"parent_id":"TOP_LEVEL_COMMENT_ID","max_results":100}` and follow its page tokens to obtain every page. To post a reply use `responder_comentario` with `{"parent_id":"TOP_LEVEL_COMMENT_ID","text":"Thank you!"}`. Replies are real public actions; after an uncertain network failure, check the thread before retrying.

### `obter_link_compartilhamento`

```json
{"resource":"short","id":"VIDEO_ID_11"}
```

`resource` accepts `video`, `short`, or `playlist`. The URL is generated locally; the tool does not check that the resource exists, that it is accessible, or that YouTube classified it as a Short.

## Examples of usage with an agent

- “Show the channel connected to Berry.”
- “List the first page of uploads and tell me whether there is another page.”
- “Upload this local video as private, with the title I provided.”
- “Schedule this video for the exact date and time I provided.”
- “Mark the specified video as containing realistic altered or synthetic media.”
- “Set this JPEG as the thumbnail for the specified video.”

Write operations make real changes to the authorized channel. Verify the actual API response and status before reporting that a video or post was published or scheduled.
