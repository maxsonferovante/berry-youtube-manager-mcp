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
  "contains_synthetic_media":true
}
```

Only `file_path`, `title`, and `made_for_kids` are required. Allowed privacy values: `public`, `private`, `unlisted`. Without a schedule, omitted privacy defaults to public. `publish_at` requires private privacy and uses RFC 3339 with an offset. The category defaults to `10`. Omitting `contains_synthetic_media` sends no declaration; `false` sends an explicit negative declaration.

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

### `publicar_post`

```json
{"channel_id":"CHANNEL_ID","text":"Hello, community!"}
```

An optional `image_paths` array can contain up to ten absolute JPEG, PNG, GIF or WebP paths; text or at least one image is required. A successful call publishes immediately and publicly. The API is a [restricted pilot](https://developers.google.com/youtube/data/reference/rest/v3/posts/insert), and OAuth approval alone does not guarantee access. It does not support scheduling posts. A YouTube-related-video link for a Short is not exposed here; follow the [Studio instructions](https://support.google.com/youtube/answer/14075157).

## Examples of usage with an agent

- “Show the channel connected to Berry.”
- “List the first page of uploads and tell me whether there is another page.”
- “Upload this local video as private, with the title I provided.”
- “Schedule this video for the exact date and time I provided.”
- “Mark the specified video as containing realistic altered or synthetic media.”
- “Set this JPEG as the thumbnail for the specified video.”

Write operations make real changes to the authorized channel. Verify the actual API response and status before reporting that a video or post was published or scheduled.
