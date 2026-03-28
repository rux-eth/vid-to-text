# PR-009: File Transfer

## Scope

Upload mp4 from client to server, download JSON result back.

- Client uploads mp4 file as multipart HTTP POST to server
- Server stores uploaded file in a temp directory for processing
- On completion, client downloads JSON result from server
- Cleanup: server removes uploaded file and temp artifacts after result is retrieved
- Handle large files (1GB+): streaming upload/download, no full-file buffering in memory
- Configurable: max upload size, temp directory path

## Dependencies

PR-008

## Verification Criteria

- A 100MB+ file uploads successfully without OOM
- Upload progress is reported to the user
- Downloaded JSON result matches what the server produced
- Server cleans up temp files after result retrieval
- Upload of file exceeding max size produces clear error
- Network interruption during upload produces clear error (not a hang)
