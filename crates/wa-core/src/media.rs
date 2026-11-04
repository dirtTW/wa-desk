//! media operations for `Session`.

use super::*;

impl Session {
    /// Upload a local file and send it as an image, video or document.
    ///
    /// `payload` is standard base64 of the file bytes; the webview sends it that
    /// way because a file input does not expose a filesystem path.
    pub async fn send_attachment(
        &self,
        chat: &str,
        name: &str,
        mime: &str,
        caption: Option<&str>,
        payload: &str,
    ) -> Result<AttachmentResult, String> {
        use base64::Engine as _;

        let jid = self.resolve_chat(chat)?;
        let data = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(|e| format!("invalid attachment payload: {e}"))?;
        if data.is_empty() {
            return Err("the selected file is empty".into());
        }

        let kind = if mime.starts_with("image/") {
            "image"
        } else if mime.starts_with("video/") {
            "video"
        } else {
            "document"
        };

        let media_type = match kind {
            "image" => MediaType::Image,
            "video" => MediaType::Video,
            _ => MediaType::Document,
        };

        let upload = self
            .client
            .upload(data, media_type, Default::default())
            .await
            .map_err(|e| format!("upload failed: {e}"))?;

        let mimetype = if mime.is_empty() {
            None
        } else {
            Some(mime.to_owned())
        };
        let caption = caption
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);

        let message = match kind {
            "image" => whatsapp_rust::media::image_message(
                upload,
                whatsapp_rust::media::ImageOptions {
                    mimetype,
                    caption: caption.clone(),
                    ..Default::default()
                },
            ),
            "video" => whatsapp_rust::media::video_message(
                upload,
                whatsapp_rust::media::VideoOptions {
                    mimetype,
                    caption: caption.clone(),
                    ..Default::default()
                },
            ),
            _ => whatsapp_rust::media::document_message(
                upload,
                whatsapp_rust::media::DocumentOptions {
                    mimetype,
                    file_name: Some(name.to_owned()),
                    title: Some(name.to_owned()),
                    caption: caption.or_else(|| Some(name.to_owned())),
                    ..Default::default()
                },
            ),
        };

        let sent = self
            .client
            .send_message(jid.clone(), message.clone())
            .await
            .map_err(|e| format!("send failed: {e}"))?;

        // Keep our own upload locally: the chat then renders it from disk
        // (even after a restart) instead of re-downloading from the CDN.
        let extension = extension_for(
            Some(mime).filter(|value| !value.is_empty()),
            match kind {
                "image" => MediaType::Image,
                "video" => MediaType::Video,
                _ => MediaType::Document,
            },
        );
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(payload) {
            let _ = tokio::fs::create_dir_all(&self.media_dir).await;
            let safe = safe_media_id(&sent.message_id);
            let _ =
                tokio::fs::write(self.media_dir.join(format!("{safe}.{extension}")), bytes).await;
        }
        remember_message(
            &self.messages,
            &self.msg_dir,
            &sent.message_id,
            Arc::new(message),
            &jid,
            true,
            None,
        );

        Ok(AttachmentResult {
            id: sent.message_id,
            kind: kind.to_owned(),
        })
    }

    /// Sticker packs seen so far (they arrive when someone shares one).
    pub fn sticker_packs(&self) -> Vec<wa_types::StickerPackView> {
        crate::sticker_packs::load(&self.media_dir)
            .iter()
            .map(|pack| crate::sticker_packs::view(&self.media_dir, pack))
            .collect()
    }

    /// Download and unpack one pack's stickers, reusing the cached files.
    pub async fn fetch_sticker_pack(
        &self,
        id: &str,
    ) -> Result<Vec<wa_types::StickerItemView>, String> {
        let Some(pack) = crate::sticker_packs::load(&self.media_dir)
            .into_iter()
            .find(|pack| pack.id == id)
        else {
            return Err("unknown sticker pack".to_string());
        };
        let cached = pack
            .file_names
            .iter()
            .all(|name| crate::sticker_packs::sticker_path(&self.media_dir, &pack, name).is_file());
        if cached {
            return Ok(crate::sticker_packs::view(&self.media_dir, &pack).stickers);
        }
        if pack.direct_path.is_empty()
            || pack.media_key.is_empty()
            || pack.file_enc_sha256.is_empty()
            || pack.file_length == 0
        {
            return Err("the pack metadata is incomplete".to_string());
        }
        let params = DownloadParams {
            direct_path: pack.direct_path.clone(),
            media_key: Some(pack.media_key.clone()),
            file_sha256: pack.file_sha256.clone(),
            file_enc_sha256: Some(pack.file_enc_sha256.clone()),
            file_length: pack.file_length,
            media_type: MediaType::Sticker,
        };
        let zip = self.download(&params).await?;
        crate::sticker_packs::extract(&self.media_dir, &pack, &zip)?;
        Ok(crate::sticker_packs::view(&self.media_dir, &pack).stickers)
    }

    /// Upload a .webp file and send it as a sticker.
    /// Send a recorded voice note: Ogg/Opus bytes, its length and the 64-bar
    /// waveform the bubble draws (`ptt` = push-to-talk, like the phone).
    pub async fn send_voice_note(
        &self,
        chat: &str,
        payload: &str,
        seconds: u32,
        waveform: Vec<u8>,
    ) -> Result<String, String> {
        use base64::Engine as _;

        let jid = self.resolve_chat(chat)?;
        let data = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(|e| format!("invalid voice note payload: {e}"))?;
        if data.is_empty() {
            return Err("the voice note is empty".into());
        }
        let upload = self
            .client
            .upload(data, MediaType::Audio, Default::default())
            .await
            .map_err(|e| format!("voice note upload failed: {e}"))?;
        let message = whatsapp_rust::media::audio_message(
            upload,
            whatsapp_rust::media::AudioOptions {
                mimetype: Some("audio/ogg; codecs=opus".to_string()),
                duration_seconds: Some(seconds),
                ptt: Some(true),
                waveform: Some(waveform),
                ..Default::default()
            },
        );
        let sent = self
            .client
            .send_message(jid, message)
            .await
            .map_err(|e| format!("could not send the voice note: {e}"))?;
        Ok(sent.message_id)
    }

    pub async fn send_sticker(
        &self,
        chat: &str,
        payload: &str,
    ) -> Result<AttachmentResult, String> {
        use base64::Engine as _;

        let jid = self.resolve_chat(chat)?;
        let data = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .map_err(|e| format!("invalid sticker payload: {e}"))?;
        if data.is_empty() {
            return Err("the selected sticker is empty".into());
        }

        let (width, height, animated) = webp_size(&data).unwrap_or((512, 512, false));

        let upload = self
            .client
            .upload(data, MediaType::Sticker, Default::default())
            .await
            .map_err(|e| format!("sticker upload failed: {e}"))?;

        let message = wa::Message {
            sticker_message: MessageField::some(wa::message::StickerMessage {
                url: Some(upload.url),
                file_sha256: Some(upload.file_sha256.to_vec()),
                file_enc_sha256: Some(upload.file_enc_sha256.to_vec()),
                media_key: Some(upload.media_key.to_vec()),
                direct_path: Some(upload.direct_path),
                file_length: Some(upload.file_length),
                media_key_timestamp: Some(upload.media_key_timestamp),
                mimetype: Some("image/webp".to_owned()),
                width: Some(width),
                height: Some(height),
                is_animated: Some(animated),
                ..Default::default()
            }),
            ..Default::default()
        };

        let sent = self
            .client
            .send_message(jid.clone(), message.clone())
            .await
            .map_err(|e| format!("send failed: {e}"))?;

        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(payload) {
            let _ = tokio::fs::create_dir_all(&self.media_dir).await;
            let safe = safe_media_id(&sent.message_id);
            let _ = tokio::fs::write(self.media_dir.join(format!("{safe}.webp")), bytes).await;
        }
        remember_message(
            &self.messages,
            &self.msg_dir,
            &sent.message_id,
            Arc::new(message),
            &jid,
            true,
            None,
        );

        Ok(AttachmentResult {
            id: sent.message_id,
            kind: "sticker".to_owned(),
        })
    }

    /// Re-send a sticker that is already in the local cache.
    pub async fn forward_sticker(&self, chat: &str, message_id: &str) -> Result<(), String> {
        let jid = self.resolve_chat(chat)?;
        let cached = {
            let mut cache = self.messages.lock().expect("media lock");
            match cache.get(message_id).cloned() {
                Some(entry) => Some(entry),
                None => load_media(&self.msg_dir, message_id).inspect(|entry| {
                    cache.insert(message_id.to_owned(), entry.clone());
                }),
            }
        }
        .ok_or_else(|| "that sticker is no longer cached".to_string())?;

        self.client
            .forward_message(jid, &cached.message)
            .await
            .map_err(|e| format!("send failed: {e}"))?;
        Ok(())
    }

    /// Fetch a channel post's plaintext CDN object. Newsletter media is not
    /// E2E encrypted, so there is nothing to decrypt — just store the bytes.
    async fn fetch_plain(&self, url: &str) -> Result<Vec<u8>, String> {
        let mut response = ureq::get(url)
            .call()
            .map_err(|err| format!("media request failed: {err}"))?;
        response
            .body_mut()
            .with_config()
            .limit(64 * 1024 * 1024)
            .read_to_vec()
            .map_err(|err| format!("media body unreadable: {err}"))
    }

    /// Download and decrypt the media of a message the UI is showing.
    /// Returns the absolute path of the file inside the media cache directory.
    /// Already-downloaded media is served from disk.
    pub async fn fetch_media(&self, message_id: &str) -> Result<String, String> {
        // Already downloaded by an earlier run: the file name carries the id.
        let safe = safe_media_id(message_id);
        for extension in MEDIA_EXTENSIONS {
            let candidate = self.media_dir.join(format!("{safe}.{extension}"));
            if candidate.exists() {
                return Ok(candidate.to_string_lossy().into_owned());
            }
        }
        let cached = {
            let mut cache = self.messages.lock().expect("media lock");
            match cache.get(message_id).cloned() {
                Some(entry) => Some(entry),
                None => load_media(&self.msg_dir, message_id).inspect(|entry| {
                    cache.insert(message_id.to_owned(), entry.clone());
                }),
            }
        }
        .ok_or_else(|| "media is no longer cached — reopen the chat".to_string())?;

        let (mut params, extension) = match media_params(&cached.message) {
            Some(parts) => parts,
            None => {
                // Channel post: plaintext CDN object, no Signal keys. Some
                // newsletter payloads ship only `directPath` (no inline URL,
                // no media key), so build the plain URL from the media
                // connection's auth token — the same access every object gets.
                let (url, extension) = match plain_media_url(&cached.message) {
                    Some(parts) => parts,
                    None => {
                        // First the key-path shape (direct path alone on a
                        // newsletter object), then give up.
                        let (direct_path, media_type, mimetype) =
                            match plain_direct_media(&cached.message) {
                                Some(parts) => parts,
                                None => {
                                    let raw = raw_media(&cached.message).ok_or_else(|| {
                                        "this message has no downloadable media".to_string()
                                    })?;
                                    (raw.direct_path.to_owned(), raw.media_type, raw.mimetype)
                                }
                            };
                        let conn = self
                            .client
                            .refresh_media_conn(false)
                            .await
                            .map_err(|err| format!("cannot refresh media connection: {err}"))?;
                        let host = conn
                            .hosts
                            .first()
                            .ok_or_else(|| "no media host".to_string())?
                            .hostname
                            .clone();
                        (
                            format!("https://{host}{direct_path}?auth={}", conn.auth),
                            extension_for(mimetype, media_type),
                        )
                    }
                };
                let file = self.media_dir.join(format!("{safe}.{extension}"));
                if !file.exists() {
                    let bytes = self.fetch_plain(&url).await?;
                    std::fs::write(&file, &bytes)
                        .map_err(|err| format!("cannot save media: {err}"))?;
                }
                return Ok(file.to_string_lossy().into_owned());
            }
        };

        let file = self.media_dir.join(format!("{safe}.{extension}"));
        if file.exists() {
            return Ok(file.to_string_lossy().into_owned());
        }

        let mut bytes = self.download(&params).await;

        // A history photo/video usually points at a CDN path the server has
        // already expired. Ask WhatsApp to re-upload it and retry once; this is
        // what WhatsApp Web does behind its "downloading" spinner.
        if bytes.is_err()
            && let Some(direct_path) = self.request_media_reupload(message_id, &cached).await
        {
            // The re-uploaded path carries its own signature query
            // (`?ccb=…&oh=…&oe=…`). The stock downloader assumes a bare path
            // and appends `?auth=<auth>&token=<token>`, so the second `?` silently
            // fetches the wrong object (media MAC mismatch). Merge the parameters
            // with `&` through a static-URL download instead.
            if let Some(raw) = raw_media(&cached.message)
                && let Some(fresh) = self.download_reuploaded(&direct_path, &raw).await
            {
                bytes = Ok(fresh);
            }
            if bytes.is_err() {
                params.direct_path = direct_path
                    .split('?')
                    .next()
                    .unwrap_or(direct_path.as_str())
                    .to_owned();
                bytes = self.download(&params).await;
            }
        }

        let bytes = bytes?;

        tokio::fs::create_dir_all(&self.media_dir)
            .await
            .map_err(|e| format!("cannot create media cache: {e}"))?;
        tokio::fs::write(&file, bytes)
            .await
            .map_err(|e| format!("cannot write media: {e}"))?;

        Ok(file.to_string_lossy().into_owned())
    }

    /// Path of media that is already on disk, without starting a download.
    /// Used to sample a video's first frame for clients whose history entry
    /// has no embedded JPEG poster.
    pub fn cached_media_path(&self, message_id: &str) -> Option<String> {
        let safe = safe_media_id(message_id);
        for extension in MEDIA_EXTENSIONS {
            let candidate = self.media_dir.join(format!("{safe}.{extension}"));
            if candidate.exists() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
        None
    }

    /// Inline JPEG thumbnail shipped with a photo/video message. Useful when a
    /// full video cannot be decoded (for example a GIF without ffmpeg).
    pub fn message_thumbnail(&self, message_id: &str) -> Option<String> {
        use base64::Engine as _;

        let cached = {
            let mut cache = self.messages.lock().expect("media lock");
            match cache.get(message_id).cloned() {
                Some(entry) => Some(entry),
                None => load_media(&self.msg_dir, message_id).inspect(|entry| {
                    cache.insert(message_id.to_owned(), entry.clone());
                }),
            }
        }?;
        let base = cached.message.get_base_message();
        let bytes = base
            .image_message
            .as_option()
            .and_then(|image| image.jpeg_thumbnail.as_deref())
            .or_else(|| {
                base.video_message
                    .as_option()
                    .and_then(|video| video.jpeg_thumbnail.as_deref())
            })
            .or_else(|| match template_media(&cached.message)? {
                TemplateMedia::Image(image) => image.jpeg_thumbnail.as_deref(),
                TemplateMedia::Video(video) => video.jpeg_thumbnail.as_deref(),
                TemplateMedia::Document(document) => document.jpeg_thumbnail.as_deref(),
            })
            .filter(|bytes| !bytes.is_empty())?;
        Some(format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    }

    /// Fetch the large link-preview thumbnail that channel/broadcast link posts
    ///
    /// The object is a plain (unencrypted) image served with the same `auth`
    /// token as every other media object; WA Web displays it directly.
    /// Returns a data URI so the webview can render it like an inline thumb.
    /// The bytes are cached on disk (`lthumb-…`), so reopening the channel is
    /// instant instead of re-downloading every post's image.
    pub async fn link_thumbnail(&self, message_id: &str) -> Result<Option<String>, String> {
        let cached = {
            let cache = self.messages.lock().expect("media lock");
            cache
                .get(message_id)
                .cloned()
                .or_else(|| load_media(&self.msg_dir, message_id))
        };
        let Some(cached) = cached else {
            return Ok(None);
        };
        let Some(thumb_path) = link_preview(&cached.message).and_then(|link| link.thumb_path)
        else {
            return Ok(None);
        };

        let safe = format!("lthumb-{}", safe_media_id(message_id));
        let disk_file = self.media_dir.join(&safe);
        if disk_file.exists()
            && let Ok(bytes) = tokio::fs::read(&disk_file).await
        {
            return Ok(Some(data_uri_for_thumbs(&bytes)));
        }

        let conn = self
            .client
            .refresh_media_conn(false)
            .await
            .map_err(|err| format!("cannot refresh media connection: {err}"))?;
        let host = conn
            .hosts
            .first()
            .ok_or_else(|| "no media host".to_string())?
            .hostname
            .clone();
        let url = format!("https://{host}{thumb_path}?auth={}", conn.auth);
        let bytes = tokio::task::spawn_blocking(move || {
            let mut response = ureq::get(&url)
                .call()
                .map_err(|err| format!("thumbnail request failed: {err}"))?;
            response
                .body_mut()
                .with_config()
                .limit(8 * 1024 * 1024)
                .read_to_vec()
                .map_err(|err| format!("thumbnail body unreadable: {err}"))
        })
        .await
        .map_err(|err| err.to_string())??;
        let _ = tokio::fs::write(&disk_file, &bytes).await;
        Ok(Some(data_uri_for_thumbs(&bytes)))
    }

    /// File-card metadata for a cached document message (name, MIME, size,
    /// pages and the embedded JPEG thumbnail). Lets the UI retrofit chats that
    /// were stored before documents rendered as a file card.
    pub fn message_document(&self, message_id: &str) -> Option<DocumentView> {
        let cached = {
            let mut cache = self.messages.lock().expect("media lock");
            match cache.get(message_id).cloned() {
                Some(entry) => Some(entry),
                None => load_media(&self.msg_dir, message_id).inspect(|entry| {
                    cache.insert(message_id.to_owned(), entry.clone());
                }),
            }
        }?;
        document_view(&cached.message)
    }

    /// Resync the app-state `regular_low` collection and download every
    /// `favoriteSticker` record through the linked session.
    pub async fn fetch_favorite_stickers(&self) -> Result<Vec<FavoriteStickerView>, String> {
        let mut records = whatsapp_rust::sticker_favorites::snapshot();
        let mut existing = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir(&self.media_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with("fav-") && name.ends_with(".webp") {
                    existing.push(FavoriteStickerView {
                        id: name.trim_end_matches(".webp").to_owned(),
                        path: entry.path().to_string_lossy().into_owned(),
                        width: 512,
                        height: 512,
                    });
                }
            }
        }
        if !existing.is_empty() {
            return Ok(existing);
        }
        if records.is_empty() {
            if let Err(err) = self.client.resync_favorite_stickers().await {
                tracing::warn!(%err, "favorite sticker sync failed");
            }
            records = whatsapp_rust::sticker_favorites::snapshot();
        }
        if records.is_empty() {
            return Ok(Vec::new());
        }

        let mut out = Vec::new();
        for record in records {
            let safe: String = record
                .id
                .chars()
                .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
                .take(72)
                .collect();
            if safe.is_empty() {
                continue;
            }
            let path = self.media_dir.join(format!("fav-{safe}.webp"));
            if path.exists() {
                out.push(FavoriteStickerView {
                    id: format!("fav-{safe}"),
                    path: path.to_string_lossy().into_owned(),
                    width: record.width,
                    height: record.height,
                });
                continue;
            }

            let Some(direct_path) = record.direct_path.clone() else {
                continue;
            };
            let Some(media_key) = record.media_key.clone() else {
                continue;
            };
            let Some(file_length) = record.file_length else {
                continue;
            };
            let Some(file_enc_sha256) = record.file_enc_sha256.clone() else {
                continue;
            };
            let file_sha256 = record.file_sha256.clone().unwrap_or_default();
            let params = DownloadParams {
                direct_path,
                media_key: Some(media_key),
                file_sha256,
                file_enc_sha256: Some(file_enc_sha256),
                file_length,
                media_type: MediaType::Sticker,
            };
            let bytes = match self.download(&params).await {
                Ok(bytes) => bytes,
                Err(err) => {
                    tracing::warn!(id = %record.id, %err, "favorite sticker download failed");
                    continue;
                }
            };

            tokio::fs::create_dir_all(&self.media_dir)
                .await
                .map_err(|e| format!("cannot create media cache: {e}"))?;
            tokio::fs::write(&path, bytes)
                .await
                .map_err(|e| format!("cannot write favorite sticker: {e}"))?;
            out.push(FavoriteStickerView {
                id: format!("fav-{safe}"),
                path: path.to_string_lossy().into_owned(),
                width: record.width,
                height: record.height,
            });
        }
        Ok(out)
    }

    /// Send one of the downloaded favorites to a chat.
    pub async fn send_favorite_sticker(
        &self,
        chat: &str,
        id: &str,
    ) -> Result<AttachmentResult, String> {
        use base64::Engine as _;

        if id.starts_with("fav-") {
            let path = self.media_dir.join(format!("{id}.webp"));
            if path.exists() {
                let bytes = tokio::fs::read(&path)
                    .await
                    .map_err(|e| format!("cannot read favorite sticker: {e}"))?;
                let payload = base64::engine::general_purpose::STANDARD.encode(bytes);
                return self.send_sticker(chat, &payload).await;
            }
        }

        let favorites = self.fetch_favorite_stickers().await?;
        let favorite = favorites
            .into_iter()
            .find(|favorite| favorite.id == id)
            .ok_or("that favorite sticker is no longer available")?;
        let bytes = tokio::fs::read(&favorite.path)
            .await
            .map_err(|e| format!("cannot read favorite sticker: {e}"))?;
        let payload = base64::engine::general_purpose::STANDARD.encode(bytes);
        self.send_sticker(chat, &payload).await
    }

    async fn download(&self, params: &DownloadParams) -> Result<Vec<u8>, String> {
        tokio::time::timeout(Duration::from_secs(90), self.client.download(params))
            .await
            .map_err(|_| "media download timed out".to_string())?
            .map_err(|e| format!("media download failed: {e}"))
    }

    /// Download a freshly re-uploaded media object. The `mediaretry` path
    /// already contains its own signature query, so `auth`/`token` must be
    /// merged with `&` rather than appended after a second `?`.
    async fn download_reuploaded(&self, direct_path: &str, raw: &RawMedia<'_>) -> Option<Vec<u8>> {
        use base64::Engine as _;
        use whatsapp_rust::download::Downloadable;

        struct StaticMedia<'a> {
            url: String,
            media_key: &'a [u8],
            file_sha256: &'a [u8],
            file_enc_sha256: &'a [u8],
            file_length: u64,
            media_type: MediaType,
        }

        impl Downloadable for StaticMedia<'_> {
            fn direct_path(&self) -> Option<&str> {
                None
            }
            fn media_key(&self) -> Option<&[u8]> {
                Some(self.media_key)
            }
            fn file_enc_sha256(&self) -> Option<&[u8]> {
                Some(self.file_enc_sha256)
            }
            fn file_sha256(&self) -> Option<&[u8]> {
                Some(self.file_sha256)
            }
            fn file_length(&self) -> Option<u64> {
                Some(self.file_length)
            }
            fn app_info(&self) -> MediaType {
                self.media_type
            }
            fn static_url(&self) -> Option<&str> {
                Some(&self.url)
            }
        }

        let conn = self.client.refresh_media_conn(false).await.ok()?;
        let host = conn.hosts.first()?.hostname.clone();
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw.file_enc_sha256);
        let separator = if direct_path.contains('?') { '&' } else { '?' };
        let url = format!(
            "https://{host}{direct_path}{separator}auth={}&token={token}",
            conn.auth
        );
        tracing::debug!(%url, "media reupload: fetching fresh path");

        let media = StaticMedia {
            url,
            media_key: raw.media_key,
            file_sha256: raw.file_sha256,
            file_enc_sha256: raw.file_enc_sha256,
            file_length: raw.file_length,
            media_type: raw.media_type,
        };
        match self.client.download(&media).await {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                tracing::debug!(%err, "media reupload: fresh path download failed");
                None
            }
        }
    }

    /// Ask the account's primary device to re-upload expired media.
    ///
    /// Mirrors Baileys' `encryptMediaRetryRequest`: `<receipt id to type=server-error>`
    /// addressed to the **normalized** own JID (no device suffix), with the
    /// encrypted `ServerErrorReceipt` and an `<rmr>` naming the original message.
    /// The library helper addresses `<jid>:55@s.whatsapp.net`, which targets this
    /// device instead of the account, so the primary never answers.
    async fn request_media_reupload(
        &self,
        message_id: &str,
        cached: &CachedMedia,
    ) -> Option<String> {
        use whatsapp_rust::wacore::media_retry::{
            encrypt_media_retry_receipt, parse_media_retry_notification,
        };
        use whatsapp_rust::{NodeBuilder, NodeFilter};

        let chat: Jid = cached.chat.parse().ok()?;
        let raw = raw_media(&cached.message)?;
        let mut own = self
            .client
            .persistence_manager()
            .get_device_snapshot()
            .pn
            .clone()?;
        own.device = 0;
        own.agent = 0;
        // Baileys sends `participant` only when the message key carries one;
        // adding our own JID for from-me group messages makes the phone ignore
        // the receipt (verified), so leave it absent.
        let participant = cached
            .participant
            .as_deref()
            .and_then(|jid| jid.parse::<Jid>().ok());

        let (ciphertext, iv) = encrypt_media_retry_receipt(raw.media_key, message_id).ok()?;
        let mut rmr = NodeBuilder::new("rmr")
            .attr("jid", &chat)
            .attr("from_me", cached.from_me);
        if let Some(participant) = participant.as_ref() {
            rmr = rmr.attr("participant", participant);
        }
        let receipt = NodeBuilder::new("receipt")
            .attr("id", message_id)
            .attr("to", &own)
            .attr("type", "server-error")
            .children([
                NodeBuilder::new("encrypt")
                    .children([
                        NodeBuilder::new("enc_p").bytes(ciphertext).build(),
                        NodeBuilder::new("enc_iv").bytes(iv.to_vec()).build(),
                    ])
                    .build(),
                rmr.build(),
            ])
            .build();

        tracing::debug!(
            own = %own,
            chat = %chat,
            from_me = cached.from_me,
            participant = ?participant,
            length = raw.file_length,
            key_len = raw.media_key.len(),
            sha_len = raw.file_sha256.len(),
            enc_sha_len = raw.file_enc_sha256.len(),
            path = raw.direct_path,
            "media reupload request"
        );

        let waiter = self.client.wait_for_node(
            NodeFilter::tag("notification")
                .attr("type", "mediaretry")
                .attr("id", message_id),
        );
        if let Err(err) = self.client.send_node(receipt).await {
            tracing::debug!(%err, "media reupload: could not send receipt");
            return None;
        }

        let notification = tokio::time::timeout(Duration::from_secs(30), waiter)
            .await
            .ok()?
            .ok()?;
        match parse_media_retry_notification(notification.get(), raw.media_key) {
            Ok(MediaRetryResult::Success { direct_path }) => {
                tracing::info!(id = %message_id, "media reupload: fresh CDN path received");
                Some(direct_path)
            }
            Ok(other) => {
                tracing::debug!(?other, "media reupload refused");
                None
            }
            Err(err) => {
                tracing::debug!(%err, "media reupload: unreadable notification");
                None
            }
        }
    }
}

/// Sniff the image format of a downloaded link-preview thumbnail and wrap it
/// in a data URI for the webview (PNG/WEBP/GIF, default JPEG).
fn data_uri_for_thumbs(bytes: &[u8]) -> String {
    use base64::Engine as _;
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/jpeg"
    };
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}
