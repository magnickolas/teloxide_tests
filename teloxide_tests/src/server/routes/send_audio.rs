use std::{collections::HashMap, str::FromStr, sync::Mutex};

use actix_multipart::Multipart;
use actix_web::{error::ErrorBadRequest, web, Responder};
use mime::Mime;
use rand::distr::{Alphanumeric, SampleString};
use serde::Deserialize;
use teloxide::types::{
    BusinessConnectionId, EffectId, FileId, FileUniqueId, Me, MessageEntity, ParseMode,
    ReplyMarkup, ReplyParameters, Seconds,
};

use super::{get_raw_multipart_fields, make_telegram_result, BodyChatId};
use crate::{
    proc_macros::SerializeRawFields,
    server::{
        routes::{check_if_message_exists, Attachment, FileType, SerializeRawFields},
        SentMessageAudio,
    },
    state::State,
    MockMessageAudio,
};

pub async fn send_audio(
    mut payload: Multipart,
    me: web::Data<Me>,
    state: web::Data<Mutex<State>>,
) -> impl Responder {
    let (fields, attachments) = get_raw_multipart_fields(&mut payload).await;
    let mut lock = state.lock().unwrap();
    let body =
        SendMessageAudioBody::serialize_raw_fields(&fields, &attachments, FileType::Audio).unwrap();
    let chat = body.chat_id.chat();

    let mut message = MockMessageAudio::new().chat(chat.clone());
    message.has_protected_content = body.protect_content.unwrap_or(false);
    message.from = Some(me.user.clone());
    message.caption = body.caption.clone();
    message.caption_entities = body.caption_entities.clone().unwrap_or_default();
    message.effect_id = body.message_effect_id.clone();
    message.business_connection_id = body.business_connection_id.clone();

    if let Some(reply_parameters) = &body.reply_parameters {
        check_if_message_exists!(lock, reply_parameters.message_id.0);
        let reply_to_message = lock
            .messages
            .get_message(reply_parameters.message_id.0)
            .unwrap();
        message.reply_to_message = Some(Box::new(reply_to_message.clone()));
    }
    if let Some(ReplyMarkup::InlineKeyboard(markup)) = body.reply_markup.clone() {
        message.reply_markup = Some(markup);
    }

    let file_id = FileId(Alphanumeric.sample_string(&mut rand::rng(), 16));
    let file_unique_id = FileUniqueId(Alphanumeric.sample_string(&mut rand::rng(), 8));

    message.file_id = file_id;
    message.file_unique_id = file_unique_id;
    message.performer = body.performer.clone();
    message.title = body.title.clone();
    message.duration = body.duration.unwrap_or(Seconds::from_seconds(0));
    message.file_size = body.file_data.bytes().len() as u32;
    message.mime_type = Some(Mime::from_str("audio/mp3").unwrap());
    message.file_name = Some(body.file_name.clone());

    let last_id = lock.messages.max_message_id();
    let message = lock.messages.add_message(message.id(last_id + 1).build());

    lock.files.push(teloxide::types::File {
        meta: message.audio().unwrap().file.clone(),
        path: body.file_name.to_owned(),
    });
    lock.responses.sent_messages.push(message.clone());
    lock.responses.sent_messages_audio.push(SentMessageAudio {
        message: message.clone(),
        bot_request: body,
    });

    make_telegram_result(message)
}

#[derive(Debug, Clone, Deserialize, SerializeRawFields)]
pub struct SendMessageAudioBody {
    pub chat_id: BodyChatId,
    pub message_thread_id: Option<i64>,
    pub file_name: String,
    pub file_data: String,
    pub duration: Option<Seconds>,
    pub caption: Option<String>,
    pub parse_mode: Option<ParseMode>,
    pub caption_entities: Option<Vec<MessageEntity>>,
    pub performer: Option<String>,
    pub title: Option<String>,
    pub disable_notification: Option<bool>,
    pub protect_content: Option<bool>,
    pub message_effect_id: Option<EffectId>,
    pub reply_parameters: Option<ReplyParameters>,
    pub reply_markup: Option<ReplyMarkup>,
    pub business_connection_id: Option<BusinessConnectionId>,
}
