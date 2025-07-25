use std::{
    fmt::Display,
    sync::{Arc, RwLock},
    thread,
};

use chrono::Utc;
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use teloxide::{
    dispatching::{
        dialogue::{self, serializer::Json, ErasedStorage, InMemStorage, SqliteStorage, Storage},
        HandlerExt, UpdateFilterExt, UpdateHandler,
    },
    dptree::{case, deps},
    error_handlers::ErrorHandler,
    macros::BotCommands,
    net::Download,
    payloads::{BanChatMemberSetters, CopyMessageSetters, SendPhotoSetters, SendPollSetters},
    prelude::*,
    requests::Requester,
    sugar::request::RequestReplyExt,
    types::{
        BotCommand, ChatAction, ChatPermissions, DiceEmoji, InlineKeyboardButton,
        InlineKeyboardMarkup, InputFile, InputMedia, InputMediaAudio, InputMediaDocument,
        InputMediaPhoto, InputMediaVideo, LabeledPrice, LinkPreviewOptions, Message, MessageEntity,
        MessageId, PollOption, PollType, ReactionType, ReplyParameters, Update,
    },
};

use super::*;
use crate::dataset::*;

//
//
//

#[derive(Serialize, Deserialize, Clone, PartialEq, Default, Debug)]
enum State {
    #[default]
    Start,
    NotStart,
}

type MyDialogue = Dialogue<State, InMemStorage<State>>;
type ErasedDialogue = Dialogue<State, ErasedStorage<State>>;
type MyStorage = Arc<ErasedStorage<State>>;

async fn handler_with_state(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    bot.send_message(msg.chat.id, msg.text().unwrap()).await?;
    if msg.text().unwrap() == "exit" {
        dialogue.exit().await?;
        return Ok(());
    }

    dialogue.update(State::NotStart).await?;
    Ok(())
}

async fn handler_with_not_start_state(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    bot.send_message(msg.chat.id, "Not start!").await?;

    dialogue.update(State::Start).await?;
    Ok(())
}

fn get_dialogue_schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    dialogue::enter::<Update, InMemStorage<State>, State, _>().branch(
        Update::filter_message()
            .branch(case![State::NotStart].endpoint(handler_with_not_start_state))
            .endpoint(handler_with_state),
    )
}

#[tokio::test]
async fn test_echo_with_start_state() {
    let mut bot = MockBot::new(MockMessageText::new().text("test"), get_dialogue_schema());
    let storage = InMemStorage::<State>::new();
    bot.dependencies(deps![storage]);
    bot.set_state(State::Start).await;

    bot.dispatch().await;

    let last_response = bot.get_responses().sent_messages.pop().unwrap();
    let state: State = bot.get_state().await;
    assert_eq!(state, State::NotStart);

    assert_eq!(last_response.text(), Some("test"));
}

#[tokio::test]
async fn test_assert_state() {
    let mut bot = MockBot::new(MockMessageText::new().text("test"), get_dialogue_schema());
    let storage = InMemStorage::<State>::new();
    bot.dependencies(deps![storage]);
    bot.set_state(State::Start).await;

    bot.dispatch().await;

    bot.assert_state(State::NotStart).await;

    let last_response = bot.get_responses().sent_messages.pop().unwrap();
    assert_eq!(last_response.text(), Some("test"));
}

#[tokio::test]
async fn test_try_get() {
    let mut bot = MockBot::new(MockMessageText::new().text("exit"), get_dialogue_schema());
    let storage = InMemStorage::<State>::new();
    bot.dependencies(deps![storage]);
    bot.set_state(State::Start).await;

    bot.dispatch().await;

    let last_response = bot.get_responses().sent_messages.pop().unwrap();
    let state: Option<State> = bot.try_get_state().await;
    assert_eq!(state, None);

    assert_eq!(last_response.text(), Some("exit"));
}

#[tokio::test]
async fn test_echo_with_not_start_test() {
    let mut bot = MockBot::new(MockMessageText::new().text("test"), get_dialogue_schema());
    let storage = InMemStorage::<State>::new();
    bot.dependencies(deps![storage]);
    bot.set_state(State::NotStart).await;

    bot.dispatch().await;

    let last_response = bot.get_responses().sent_messages.pop().unwrap();
    let state: State = bot.get_state().await;
    assert_eq!(state, State::Start);

    assert_eq!(last_response.text(), Some("Not start!"));
}

fn get_erased_dialogue_schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>>
{
    dialogue::enter::<Update, ErasedStorage<State>, State, _>()
        .branch(Update::filter_message().endpoint(handler_with_erased_state))
}

async fn handler_with_erased_state(
    bot: Bot,
    dialogue: ErasedDialogue,
    msg: Message,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    bot.send_message(msg.chat.id, msg.text().unwrap()).await?;
    dialogue.update(State::NotStart).await?;
    Ok(())
}

#[tokio::test]
async fn test_erased_state() {
    let mut bot = MockBot::new(
        MockMessageText::new().text("test"),
        get_erased_dialogue_schema(),
    );
    let storage: MyStorage = SqliteStorage::open(":memory:", Json).await.unwrap().erase();
    bot.dependencies(deps![storage]);

    // This .dispatch is important?..
    bot.dispatch().await;
    bot.dispatch_and_check_state(State::NotStart).await;
}

//
//
//

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
pub enum AllCommands {
    #[command()]
    Echo,
    #[command()]
    Edit,
    #[command()]
    EditUnchanged,
    #[command()]
    Delete,
    #[command()]
    DeleteBatch,
    #[command()]
    EditReplyMarkup,
    #[command()]
    Photo,
    #[command()]
    Video,
    #[command()]
    Audio,
    #[command()]
    Voice,
    #[command()]
    VideoNote,
    #[command()]
    Document,
    #[command()]
    Animation,
    #[command()]
    Location,
    #[command()]
    Venue,
    #[command()]
    Contact,
    #[command()]
    Dice,
    #[command()]
    Poll,
    #[command()]
    Sticker,
    #[command()]
    MediaGroup,
    #[command()]
    Invoice,
    #[command()]
    EditCaption,
    #[command()]
    PinMessage,
    #[command()]
    ForwardMessage,
    #[command()]
    CopyMessage,
    #[command()]
    Ban,
    #[command()]
    Restrict,
    #[command()]
    ChatAction,
    #[command()]
    SetMessageReaction,
    #[command()]
    SetMyCommands,
    #[command()]
    Panic,
}

async fn handler(
    bot: Bot,
    msg: Message,
    cmd: AllCommands,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let sent_message = bot.send_message(msg.chat.id, msg.text().unwrap()).await?;
    assert_eq!(msg.text().unwrap(), sent_message.text().unwrap()); // The message actually made it through!
    let reply_options = ReplyParameters::new(msg.id);
    let link_preview_options = LinkPreviewOptions {
        is_disabled: true,
        url: None,
        prefer_small_media: false,
        prefer_large_media: false,
        show_above_text: false,
    };
    match cmd {
        AllCommands::Echo => {}
        AllCommands::Edit => {
            bot.edit_message_text(msg.chat.id, sent_message.id, "edited")
                .link_preview_options(link_preview_options)
                .await?;
        }
        AllCommands::EditUnchanged => {
            bot.edit_message_text(msg.chat.id, sent_message.id, msg.text().unwrap())
                .link_preview_options(link_preview_options)
                .await?;
        }
        AllCommands::Delete => {
            bot.delete_message(msg.chat.id, sent_message.id).await?;
        }
        AllCommands::DeleteBatch => {
            bot.delete_messages(msg.chat.id, vec![sent_message.id, MessageId(404)])
                .await?;
        }
        AllCommands::EditReplyMarkup => {
            bot.edit_message_reply_markup(msg.chat.id, sent_message.id)
                .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                    InlineKeyboardButton::callback("test", "test"),
                ]]))
                .await?;
        }
        AllCommands::Photo => {
            let photo = InputFile::memory("somedata".to_string()).file_name("test.jpg");
            bot.send_photo(msg.chat.id, photo)
                .caption("test")
                .caption_entities(vec![MessageEntity::bold(0, 3)])
                .reply_parameters(reply_options)
                .await?;
        }
        AllCommands::Video => {
            let video = InputFile::memory("somedata".to_string()).file_name("test.mp4");
            bot.send_video(msg.chat.id, video)
                .caption("test")
                .caption_entities(vec![MessageEntity::bold(0, 3)])
                .has_spoiler(true)
                .reply_parameters(reply_options)
                .await?;
        }
        AllCommands::Audio => {
            let audio = InputFile::memory("somedata".to_string()).file_name("test.mp3");
            bot.send_audio(msg.chat.id, audio)
                .caption("test")
                .caption_entities(vec![MessageEntity::bold(0, 3)])
                .reply_parameters(reply_options)
                .await?;
        }
        AllCommands::Voice => {
            let voice = InputFile::memory("somedata".to_string()).file_name("test.mp3");
            bot.send_voice(msg.chat.id, voice)
                .caption("test")
                .caption_entities(vec![MessageEntity::bold(0, 3)])
                .reply_parameters(reply_options)
                .await?;
        }
        AllCommands::VideoNote => {
            let video_note = InputFile::memory("somedata".to_string()).file_name("test.mp4");
            bot.send_video_note(msg.chat.id, video_note)
                .reply_parameters(reply_options)
                .await?;
        }
        AllCommands::EditCaption => {
            let photo = InputFile::file_id("fileid".into());
            let photo_message = bot.send_photo(msg.chat.id, photo).await?;
            bot.edit_message_caption(msg.chat.id, photo_message.id)
                .caption("edited")
                .await?;
        }
        AllCommands::Document => {
            let document = InputFile::memory("somedata".to_string()).file_name("test.txt");
            let document_message = bot
                .send_document(msg.chat.id, document)
                .caption("test")
                .caption_entities(vec![MessageEntity::bold(0, 3)])
                .reply_parameters(reply_options)
                .await?;
            let gotten_document = bot
                .get_file(document_message.document().unwrap().file.id.clone())
                .await?;
            assert!(
                gotten_document.meta.unique_id
                    == document_message.document().unwrap().file.unique_id
            );
            let mut dest = tokio::fs::File::create("test.txt").await?;

            bot.download_file(&gotten_document.path, &mut dest).await?;
            assert!(tokio::fs::read_to_string("test.txt").await.is_ok());
            tokio::fs::remove_file("test.txt").await?;
        }
        AllCommands::Animation => {
            let animation = InputFile::memory("somedata".to_string()).file_name("animation.mp4");
            bot.send_animation(msg.chat.id, animation).await?;
        }
        AllCommands::Location => {
            bot.send_location(msg.chat.id, 1.0, 1.0)
                .live_period(60.into())
                .reply_parameters(reply_options)
                .await?;
        }
        AllCommands::Venue => {
            bot.send_venue(msg.chat.id, 1.0, 1.0, "test", "test")
                .reply_parameters(reply_options)
                .await?;
        }
        AllCommands::Contact => {
            bot.send_contact(msg.chat.id, "123456789", "test")
                .reply_parameters(reply_options)
                .await?;
        }
        AllCommands::Dice => {
            bot.send_dice(msg.chat.id).await?;
        }
        AllCommands::Poll => {
            bot.send_poll(
                msg.chat.id,
                "what is test",
                vec!["test".to_string().into(), "not test".to_string().into()],
            )
            .type_(PollType::Quiz)
            .reply_parameters(reply_options)
            .explanation("because test")
            .correct_option_id(0)
            .await?;
        }
        AllCommands::Sticker => {
            let sticker = InputFile::memory("somedata".to_string()).file_name("test.webp");
            bot.send_sticker(msg.chat.id, sticker)
                .reply_parameters(reply_options)
                .await?;
        }
        AllCommands::MediaGroup => {
            let audio1 = InputFile::memory("somedata".to_string()).file_name("audio1.mp3");
            let audio2 = InputFile::memory("somedata2".to_string()).file_name("audio2.mp3");
            let media_group = vec![
                InputMedia::Audio(InputMediaAudio::new(audio1.clone()).caption("test")),
                InputMedia::Audio(InputMediaAudio::new(audio2.clone())),
            ];
            bot.send_media_group(msg.chat.id, media_group)
                .reply_parameters(reply_options.clone())
                .await?;

            let document1 = InputFile::memory("somedata".to_string()).file_name("document1.txt");
            let document2 = InputFile::memory("somedata2".to_string()).file_name("document2.txt");
            let media_group = vec![
                InputMedia::Document(InputMediaDocument::new(document1.clone()).caption("test")),
                InputMedia::Document(InputMediaDocument::new(document2.clone())),
            ];
            bot.send_media_group(msg.chat.id, media_group)
                .reply_parameters(reply_options.clone())
                .await?;

            let photo1 = InputFile::memory("somedata".to_string());
            let photo2 = InputFile::memory("somedata2".to_string());
            let media_group = vec![
                InputMedia::Photo(InputMediaPhoto::new(photo1.clone()).caption("test")),
                InputMedia::Photo(InputMediaPhoto::new(photo2.clone())),
            ];
            bot.send_media_group(msg.chat.id, media_group)
                .reply_parameters(reply_options.clone())
                .await?;

            let video1 = InputFile::memory("somedata".to_string()).file_name("video1.mp4");
            let video2 = InputFile::memory("somedata2".to_string()).file_name("video2.mp4");
            let media_group = vec![
                InputMedia::Video(InputMediaVideo::new(video1.clone()).caption("test")),
                InputMedia::Video(InputMediaVideo::new(video2.clone())),
            ];
            bot.send_media_group(msg.chat.id, media_group)
                .reply_parameters(reply_options)
                .await?;
        }
        AllCommands::PinMessage => {
            bot.pin_chat_message(msg.chat.id, sent_message.id).await?;
            bot.unpin_chat_message(msg.chat.id).await?;
            bot.unpin_all_chat_messages(msg.chat.id).await?;
        }
        AllCommands::ForwardMessage => {
            bot.forward_message(msg.chat.id, msg.chat.id, sent_message.id)
                .await?;
        }
        AllCommands::CopyMessage => {
            let document = InputFile::memory("somedata".to_string()).file_name("test.txt");
            let document_message = bot.send_document(msg.chat.id, document).await?;
            bot.copy_message(msg.chat.id, msg.chat.id, document_message.id)
                .caption("test")
                .reply_markup(InlineKeyboardMarkup::new(vec![vec![
                    InlineKeyboardButton::callback("test", "test"),
                ]]))
                .await?;
        }
        AllCommands::Ban => {
            bot.ban_chat_member(msg.chat.id, msg.from.clone().unwrap().id)
                .revoke_messages(true)
                .await?;
            // Test revoking messages
            let result = bot.delete_message(msg.chat.id, msg.id).await;
            assert!(result.is_err());
            bot.unban_chat_member(msg.chat.id, msg.from.unwrap().id)
                .await?;
        }
        AllCommands::Restrict => {
            bot.restrict_chat_member(msg.chat.id, msg.from.unwrap().id, ChatPermissions::empty())
                .await?;
        }
        AllCommands::ChatAction => {
            bot.send_chat_action(msg.chat.id, ChatAction::Typing)
                .await?;
        }
        AllCommands::SetMessageReaction => {
            bot.set_message_reaction(msg.chat.id, msg.id)
                .reaction(vec![ReactionType::Emoji {
                    emoji: "👍".to_owned(),
                }])
                .await?;
        }
        AllCommands::SetMyCommands => {
            bot.set_my_commands(vec![BotCommand {
                command: String::from("test"),
                description: String::from("test"),
            }])
            .await?;
        }
        AllCommands::Panic => {
            // This message id does not exist
            bot.send_message(msg.chat.id, "test")
                .reply_to(MessageId(344382918))
                .await?;
        }
        AllCommands::Invoice => {
            bot.send_invoice(
                msg.chat.id,
                "Absolutely Nothing",
                "Demo",
                "test_payload",
                "XTR",
                vec![LabeledPrice {
                    label: "Stars".into(),
                    amount: 1,
                }],
            )
            .await?;
        }
    }
    Ok(())
}

async fn callback_handler(
    bot: Bot,
    call: CallbackQuery,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    bot.answer_callback_query(call.id)
        .text(call.data.unwrap())
        .await?;
    Ok(())
}

fn get_schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<AllCommands>()
                .endpoint(handler),
        )
        .branch(
            Update::filter_edited_message()
                .filter_command::<AllCommands>()
                .branch(case![AllCommands::ForwardMessage].endpoint(handler)),
        )
        .branch(Update::filter_callback_query().endpoint(callback_handler))
}

#[tokio::test]
async fn test_echo() {
    let mut bot = MockBot::new(MockMessageText::new().text("/echo echo"), get_schema());

    bot.dispatch().await;

    let last_response = bot.get_responses().sent_messages.pop().unwrap();

    assert_eq!(last_response.text(), Some("/echo echo"));
}

#[tokio::test]
#[should_panic]
async fn test_panic() {
    // Nothing else should fail because this panics
    let bot = MockBot::new(MockMessageText::new().text("/panic"), get_schema());

    // To actually keep the bot in scope
    if true {
        panic!("Expected panic");
    }

    drop(bot);
}

pub struct MyErrorHandler {
    errors: Arc<RwLock<Vec<String>>>,
}

impl MyErrorHandler {
    pub fn new() -> Self {
        Self {
            errors: Arc::new(RwLock::new(vec![])),
        }
    }

    pub fn errors(&self) -> Vec<String> {
        self.errors.read().unwrap().clone()
    }
}

impl<E> ErrorHandler<E> for MyErrorHandler
where
    E: std::fmt::Debug + Display + 'static + Sync + Send,
{
    fn handle_error(self: Arc<Self>, error: E) -> BoxFuture<'static, ()> {
        thread::spawn(|| {
            respond_to_error();
        })
        .join()
        .unwrap();

        self.errors.write().unwrap().push(format!("{error:?}"));
        Box::pin(async {})
    }
}

#[tokio::main]
async fn respond_to_error() {
    let bot = Bot::from_env();
    bot.send_message(ChatId(MockUser::ID as i64), "Error detected!")
        .await
        .unwrap();
}

#[tokio::test]
async fn test_error_handler() {
    let mut bot = MockBot::new(MockMessageText::new().text("/panic"), get_schema());
    let error_handler = Arc::new(MyErrorHandler::new());
    bot.error_handler(error_handler.clone());

    bot.dispatch_and_check_last_text("Error detected!").await;

    let errors = error_handler.errors();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("Message not found"));
}

#[tokio::test]
async fn test_no_updates() {
    let empty: Vec<MockMessageDice> = vec![];
    let mut bot = MockBot::new(empty, get_schema());

    // This shouldn't panic
    bot.dispatch().await;

    // Just to test that everything is fine
    bot.update(MockMessageText::new().text("/echo echo"));
    bot.dispatch_and_check_last_text("/echo echo").await;
}

#[tokio::test]
async fn test_send_photo() {
    let mut bot = MockBot::new(MockMessageText::new().text("/photo"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_sent_photo = bot.get_responses().sent_messages_photo.pop().unwrap();
    assert_eq!(last_sent_message.caption(), Some("test"));
    assert_eq!(
        last_sent_message.reply_to_message().unwrap().text(),
        Some("/photo")
    );
    assert_eq!(last_sent_message.caption_entities().unwrap().len(), 1);
    assert_eq!(last_sent_photo.bot_request.file_name, "test.jpg");
    assert_eq!(last_sent_photo.bot_request.file_data, "somedata");
}

#[tokio::test]
async fn test_send_video() {
    let mut bot = MockBot::new(MockMessageText::new().text("/video"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_sent_video = bot.get_responses().sent_messages_video.pop().unwrap();
    assert_eq!(last_sent_message.caption(), Some("test"));
    assert_eq!(
        last_sent_message.reply_to_message().unwrap().text(),
        Some("/video")
    );
    assert_eq!(last_sent_message.caption_entities().unwrap().len(), 1);
    assert_eq!(last_sent_video.bot_request.file_name, "test.mp4");
    assert_eq!(last_sent_video.bot_request.file_data, "somedata");
}

#[tokio::test]
async fn test_send_audio() {
    let mut bot = MockBot::new(MockMessageText::new().text("/audio"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_sent_audio = bot.get_responses().sent_messages_audio.pop().unwrap();
    assert_eq!(last_sent_message.caption(), Some("test"));
    assert_eq!(
        last_sent_message.reply_to_message().unwrap().text(),
        Some("/audio")
    );
    assert_eq!(last_sent_message.caption_entities().unwrap().len(), 1);
    assert_eq!(last_sent_audio.bot_request.file_name, "test.mp3");
    assert_eq!(last_sent_audio.bot_request.file_data, "somedata");
}

#[tokio::test]
async fn test_send_voice() {
    let mut bot = MockBot::new(MockMessageText::new().text("/voice"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_sent_voice = bot.get_responses().sent_messages_voice.pop().unwrap();
    assert_eq!(last_sent_message.caption(), Some("test"));
    assert_eq!(
        last_sent_message.reply_to_message().unwrap().text(),
        Some("/voice")
    );
    assert_eq!(last_sent_message.caption_entities().unwrap().len(), 1);
    assert_eq!(last_sent_voice.bot_request.file_name, "test.mp3");
    assert_eq!(last_sent_voice.bot_request.file_data, "somedata");
}

#[tokio::test]
async fn test_send_video_note() {
    let mut bot = MockBot::new(MockMessageText::new().text("/videonote"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_sent_video_note = bot.get_responses().sent_messages_video_note.pop().unwrap();
    assert_eq!(
        last_sent_message.reply_to_message().unwrap().text(),
        Some("/videonote")
    );
    assert_eq!(last_sent_video_note.bot_request.file_name, "test.mp4");
    assert_eq!(last_sent_video_note.bot_request.file_data, "somedata");
}

#[tokio::test]
async fn test_send_document() {
    let mut bot = MockBot::new(MockMessageText::new().text("/document"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_sent_photo = bot.get_responses().sent_messages_document.pop().unwrap();
    assert_eq!(last_sent_message.caption(), Some("test"));
    assert_eq!(
        last_sent_message.reply_to_message().unwrap().text(),
        Some("/document")
    );
    assert_eq!(last_sent_message.caption_entities().unwrap().len(), 1);
    assert_eq!(last_sent_photo.bot_request.file_name, "test.txt");
}

#[tokio::test]
async fn test_send_animation() {
    let mut bot = MockBot::new(MockMessageText::new().text("/animation"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_sent_animation = bot.get_responses().sent_messages_animation.pop().unwrap();
    assert_eq!(
        last_sent_message.animation().unwrap().file_name,
        Some("animation.mp4".to_string())
    );
    assert_eq!(last_sent_animation.bot_request.file_name, "animation.mp4");
}

#[tokio::test]
async fn test_send_media_group() {
    let mut bot = MockBot::new(MockMessageText::new().text("/mediagroup"), get_schema());

    bot.dispatch().await;

    let responses = bot.get_responses();

    let audio_group = responses.sent_media_group[0].clone();
    assert_eq!(
        audio_group.messages.first().unwrap().caption(),
        Some("test")
    );
    assert_eq!(
        audio_group
            .messages
            .first()
            .unwrap()
            .audio()
            .unwrap()
            .file_name,
        Some("audio1.mp3".to_string())
    );
    assert_eq!(
        audio_group
            .messages
            .first()
            .unwrap()
            .reply_to_message()
            .unwrap()
            .text(),
        Some("/mediagroup")
    );
    assert_eq!(audio_group.bot_request.media.len(), 2);

    let document_group = responses.sent_media_group[1].clone();
    assert_eq!(
        document_group.messages.first().unwrap().caption(),
        Some("test")
    );
    assert_eq!(
        document_group
            .messages
            .first()
            .unwrap()
            .document()
            .unwrap()
            .file_name,
        Some("document1.txt".to_string())
    );
    assert_eq!(
        document_group
            .messages
            .first()
            .unwrap()
            .reply_to_message()
            .unwrap()
            .text(),
        Some("/mediagroup")
    );
    assert_eq!(document_group.bot_request.media.len(), 2);

    let photo_group = responses.sent_media_group[2].clone();
    assert_eq!(
        photo_group.messages.first().unwrap().caption(),
        Some("test")
    );
    assert!(!photo_group
        .messages
        .first()
        .unwrap()
        .photo()
        .unwrap()
        .is_empty());
    assert_eq!(
        photo_group
            .messages
            .first()
            .unwrap()
            .reply_to_message()
            .unwrap()
            .text(),
        Some("/mediagroup")
    );
    assert_eq!(photo_group.bot_request.media.len(), 2);

    let video_group = responses.sent_media_group[3].clone();
    assert_eq!(
        video_group.messages.first().unwrap().caption(),
        Some("test")
    );
    assert_eq!(
        video_group
            .messages
            .first()
            .unwrap()
            .video()
            .unwrap()
            .file_name,
        Some("video1.mp4".to_string())
    );
    assert_eq!(
        video_group
            .messages
            .first()
            .unwrap()
            .reply_to_message()
            .unwrap()
            .text(),
        Some("/mediagroup")
    );
    assert_eq!(video_group.bot_request.media.len(), 2);
}

#[tokio::test]
async fn test_send_location() {
    let mut bot = MockBot::new(MockMessageText::new().text("/location"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_sent_location = bot.get_responses().sent_messages_location.pop().unwrap();
    assert_eq!(
        last_sent_message.reply_to_message().unwrap().text(),
        Some("/location")
    );
    assert_eq!(last_sent_message.location().unwrap().latitude, 1.0);
    assert_eq!(last_sent_message.location().unwrap().longitude, 1.0);
    assert_eq!(last_sent_location.bot_request.live_period, Some(60.into()));
}

#[tokio::test]
async fn test_send_venue() {
    let mut bot = MockBot::new(MockMessageText::new().text("/venue"), get_schema());

    bot.dispatch().await;

    let last_sent_venue = bot.get_responses().sent_messages_venue.pop().unwrap();
    let last_sent_message = last_sent_venue.message;
    assert_eq!(
        last_sent_message.reply_to_message().unwrap().text(),
        Some("/venue")
    );
    assert_eq!(last_sent_message.venue().unwrap().location.latitude, 1.0);
    assert_eq!(last_sent_message.venue().unwrap().location.longitude, 1.0);
    assert_eq!(last_sent_message.venue().unwrap().title, "test");
    assert_eq!(last_sent_message.venue().unwrap().address, "test");
}

#[tokio::test]
async fn test_send_contact() {
    let mut bot = MockBot::new(MockMessageText::new().text("/contact"), get_schema());

    bot.dispatch().await;

    let last_sent_contact = bot.get_responses().sent_messages_contact.pop().unwrap();
    let last_sent_message = last_sent_contact.message;
    assert_eq!(
        last_sent_message.reply_to_message().unwrap().text(),
        Some("/contact")
    );
    assert_eq!(
        last_sent_message.contact().unwrap().phone_number,
        "123456789"
    );
    assert_eq!(last_sent_message.contact().unwrap().first_name, "test");
}

#[tokio::test]
async fn test_send_dice() {
    let mut bot = MockBot::new(MockMessageText::new().text("/dice"), get_schema());

    bot.dispatch().await;

    let last_sent_contact = bot.get_responses().sent_messages_dice.pop().unwrap();
    let last_sent_message = last_sent_contact.message;
    assert_eq!(last_sent_message.dice().unwrap().emoji, DiceEmoji::Dice);
    assert!(last_sent_message.dice().unwrap().value < 100);
}

#[tokio::test]
async fn test_send_poll() {
    let mut bot = MockBot::new(MockMessageText::new().text("/poll"), get_schema());

    bot.dispatch().await;

    let last_sent_contact = bot.get_responses().sent_messages_poll.pop().unwrap();
    let last_sent_message = last_sent_contact.message;
    assert_eq!(
        last_sent_message.reply_to_message().unwrap().text(),
        Some("/poll")
    );
    assert_eq!(last_sent_message.poll().unwrap().question, "what is test");
    assert_eq!(
        last_sent_message.poll().unwrap().options,
        vec![
            PollOption {
                text: "test".to_string(),
                text_entities: None,
                voter_count: 0
            },
            PollOption {
                text: "not test".to_string(),
                text_entities: None,
                voter_count: 0
            }
        ],
    );
    assert_eq!(
        last_sent_message.poll().unwrap().explanation,
        Some("because test".to_string())
    );
    assert_eq!(last_sent_message.poll().unwrap().poll_type, PollType::Quiz);
    assert_eq!(last_sent_message.poll().unwrap().correct_option_id, Some(0));
}

#[tokio::test]
async fn test_send_sticker() {
    let mut bot = MockBot::new(MockMessageText::new().text("/sticker"), get_schema());

    bot.dispatch().await;

    let last_sent_contact = bot.get_responses().sent_messages_sticker.pop().unwrap();
    let last_sent_message = last_sent_contact.message;
    assert_eq!(
        last_sent_message.reply_to_message().unwrap().text(),
        Some("/sticker")
    );
    assert_eq!(last_sent_message.sticker().unwrap().emoji, None);
}

#[tokio::test]
async fn test_edit_message() {
    let mut bot = MockBot::new(MockMessageText::new().text("/edit"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_edited_response = bot.get_responses().edited_messages_text.pop().unwrap();

    assert_eq!(last_sent_message.text(), Some("/edit"));
    assert_eq!(last_edited_response.message.text(), Some("edited"));
    assert_eq!(
        last_edited_response
            .bot_request
            .link_preview_options
            .unwrap()
            .is_disabled,
        true
    );
}

#[tokio::test]
async fn test_edit_message_unchanged() {
    let mut bot = MockBot::new(MockMessageText::new().text("/editunchanged"), get_schema());
    let error_handler = Arc::new(MyErrorHandler::new());
    bot.error_handler(error_handler.clone());

    bot.dispatch().await;

    assert!(bot.get_responses().edited_messages_text.is_empty());
    let errors = error_handler.errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0], "Api(MessageNotModified)");
}

#[tokio::test]
async fn test_edit_caption() {
    let mut bot = MockBot::new(MockMessageText::new().text("/editcaption"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_edited_response = bot.get_responses().edited_messages_caption.pop().unwrap();

    assert_eq!(last_sent_message.caption(), None);
    assert_eq!(last_edited_response.message.caption(), Some("edited"));
}

#[tokio::test]
async fn test_edit_reply_markup() {
    let mut bot = MockBot::new(
        MockMessageText::new().text("/editreplymarkup"),
        get_schema(),
    );

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_edited_response = bot
        .get_responses()
        .edited_messages_reply_markup
        .pop()
        .unwrap();

    assert_eq!(last_sent_message.reply_markup(), None);
    assert_eq!(
        last_edited_response
            .message
            .reply_markup()
            .unwrap()
            .inline_keyboard[0][0]
            .text,
        "test"
    );
}

#[tokio::test]
async fn test_delete_message() {
    let mut bot = MockBot::new(MockMessageText::new().text("/delete"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_deleted_response = bot.get_responses().deleted_messages.pop().unwrap();

    assert_eq!(last_sent_message.text(), Some("/delete"));
    assert_eq!(last_deleted_response.message.id, last_sent_message.id);
}

#[tokio::test]
async fn test_delete_messages() {
    let mut bot = MockBot::new(MockMessageText::new().text("/deletebatch"), get_schema());

    bot.dispatch().await;

    let last_sent_message = bot.get_responses().sent_messages.pop().unwrap();
    let last_deleted_response = bot.get_responses().deleted_messages.pop().unwrap();

    assert_eq!(last_sent_message.text(), Some("/deletebatch"));
    assert_eq!(last_deleted_response.message.id, last_sent_message.id);
}

#[tokio::test]
async fn test_answer_callback_query() {
    let mut bot = MockBot::new(MockCallbackQuery::new().data("test"), get_schema());

    bot.dispatch().await;

    let answered_callback = bot.get_responses().answered_callback_queries.pop().unwrap();

    assert_eq!(answered_callback.text, Some("test".to_string()));
}

#[tokio::test]
async fn test_pin_message() {
    let mut bot = MockBot::new(MockMessageText::new().text("/pinmessage"), get_schema());

    bot.dispatch().await;

    let pinned_message = bot.get_responses().pinned_chat_messages.pop();
    let unpinned_message = bot.get_responses().unpinned_chat_messages.pop();
    let unpinned_all_chat_messages = bot.get_responses().unpinned_all_chat_messages.pop();

    assert!(pinned_message.is_some());
    assert!(unpinned_message.is_some());
    assert!(unpinned_all_chat_messages.is_some());
}

#[tokio::test]
async fn test_forward_message() {
    let mut bot = MockBot::new(MockMessageText::new().text("/forwardmessage"), get_schema());

    bot.dispatch().await;

    let responses = bot.get_responses();
    let first_sent_message = responses.sent_messages.first().unwrap();
    let last_sent_message = responses.sent_messages.last().unwrap();

    assert_eq!(last_sent_message.text(), Some("/forwardmessage"));
    assert_eq!(
        last_sent_message.forward_date(),
        Some(first_sent_message.date)
    );
    assert_eq!(
        last_sent_message.forward_from_user().unwrap().id,
        first_sent_message.from.as_ref().unwrap().id
    );
    assert_eq!(responses.forwarded_messages.len(), 1);
    assert_eq!(&responses.forwarded_messages[0].message, last_sent_message);
}

#[tokio::test]
async fn test_copy_message() {
    let mut bot = MockBot::new(MockMessageText::new().text("/copymessage"), get_schema());

    bot.dispatch().await;

    let responses = bot.get_responses();
    let second_sent_message = responses.sent_messages.get(1).unwrap();
    let last_sent_message = responses.sent_messages.last().unwrap();

    assert!(second_sent_message.caption().is_none());
    assert!(last_sent_message.document().is_some());
    assert_eq!(last_sent_message.caption(), Some("test"));
    assert_eq!(
        last_sent_message.reply_markup().unwrap().inline_keyboard[0][0].text,
        "test"
    );
}

#[tokio::test]
async fn test_ban_and_unban() {
    let mut bot = MockBot::new(MockMessageText::new().text("/ban"), get_schema());

    bot.dispatch().await;

    let responses = bot.get_responses();
    let banned_user = responses.banned_chat_members.last().unwrap();
    let unbanned_user = responses.unbanned_chat_members.last().unwrap();

    assert_eq!(banned_user.user_id, MockUser::ID);
    assert_eq!(unbanned_user.user_id, MockUser::ID);
}

#[tokio::test]
async fn test_restrict() {
    let mut bot = MockBot::new(MockMessageText::new().text("/restrict"), get_schema());

    bot.dispatch().await;

    let responses = bot.get_responses();
    let restricted_user = responses.restricted_chat_members.last().unwrap();

    assert_eq!(restricted_user.user_id, MockUser::ID);
    assert_eq!(restricted_user.permissions, ChatPermissions::empty());
}

#[tokio::test]
async fn test_send_chat_action() {
    let mut bot = MockBot::new(MockMessageText::new().text("/chataction"), get_schema());

    bot.dispatch().await;

    let responses = bot.get_responses();
    let last_chat_action = responses.sent_chat_actions.last().unwrap();

    assert_eq!(last_chat_action.action, "typing");
}

#[tokio::test]
async fn test_set_message_reaction() {
    let mut bot = MockBot::new(
        MockMessageText::new().text("/setmessagereaction"),
        get_schema(),
    );

    bot.dispatch().await;

    let responses = bot.get_responses();
    let last_reaction = responses.set_message_reaction.last().unwrap();

    assert_eq!(
        last_reaction.reaction.clone().unwrap()[0],
        ReactionType::Emoji {
            emoji: "👍".to_owned()
        }
    );
}

#[tokio::test]
async fn test_set_my_commands() {
    let mut bot = MockBot::new(MockMessageText::new().text("/setmycommands"), get_schema());

    bot.dispatch().await;

    let responses = bot.get_responses();
    let set_commands = responses.set_my_commands.last().unwrap();

    assert_eq!(
        set_commands.commands.first(),
        Some(&BotCommand {
            command: String::from("test"),
            description: String::from("test")
        })
    );
}

#[tokio::test]
async fn test_send_invoice() {
    let mut bot = MockBot::new(MockMessageText::new().text("/invoice"), get_schema());

    bot.dispatch().await;

    let responses = bot.get_responses();
    let invoice_message = responses.sent_messages_invoice.last().unwrap();

    assert_eq!(
        invoice_message.message.invoice().unwrap().title,
        "Absolutely Nothing"
    );
}

#[tokio::test]
async fn test_edited_message() {
    let mock_message = MockMessageText::new().text("/forwardmessage first");
    let mut bot = MockBot::new(mock_message.clone(), get_schema());
    bot.dispatch().await;

    let responses = bot.get_responses();
    assert_eq!(responses.forwarded_messages.len(), 1);
    let forwarded_message = &responses.forwarded_messages[0].message;
    assert_eq!(forwarded_message.text(), Some("/forwardmessage first"));

    let edited_message = MockEditedMessage::new(
        mock_message
            .text("/forwardmessage second")
            .edit_date(Utc::now())
            .build(),
    );
    bot.update(edited_message);
    bot.dispatch().await;

    let responses = bot.get_responses();
    assert_eq!(responses.forwarded_messages.len(), 1);
    let forwarded_message = &responses.forwarded_messages[0].message;
    assert_eq!(forwarded_message.text(), Some("/forwardmessage second"));
}
