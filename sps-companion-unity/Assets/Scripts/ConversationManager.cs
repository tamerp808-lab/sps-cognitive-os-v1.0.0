// SPS Emotional Companion — Conversation Manager
// Phase 14: Orchestrates STT → LLM → TTS pipeline with emotion tracking

using System;
using System.Collections;
using System.Text;
using UnityEngine;
using UnityEngine.Networking;

namespace SPS.Companion
{
    /// <summary>
    /// Main conversation orchestrator. Connects:
    /// STT (voice input) → SPS Kernel (LLM reasoning) → TTS (voice output)
    /// with real-time emotion updates via EmotionManager.
    /// </summary>
    public class ConversationManager : MonoBehaviour
    {
        [Header("Components")]
        public SttHandler Stt;
        public TtsHandler Tts;
        public EmotionManager Emotion;
        public CompanionController Companion;

        [Header("SPS Connection")]
        public string SpsServerUrl = "http://localhost:7780";

        [Header("Conversation State")]
        public bool IsInConversation = false;
        public bool AutoListenAfterSpeak = true;

        private string _lastUserMessage = "";
        private string _lastAssistantMessage = "";

        void Start()
        {
            if (Stt != null)
            {
                Stt.OnTranscriptionReceived += OnTranscriptionReceived;
                Stt.OnRecordingStarted += () =>
                {
                    Emotion?.SetListening();
                };
                Stt.OnRecordingStopped += () =>
                {
                    Emotion?.SetThinking();
                };
            }
        }

        /// <summary>
        /// Start a conversation — begin listening for user input.
        /// </summary>
        public void StartConversation()
        {
            IsInConversation = true;
            Emotion?.LockEmotion(CompanionEmotion.Listening);
            Stt?.StartRecording();
            Debug.Log("[SPS Conv] Conversation started");
        }

        /// <summary>
        /// End the conversation.
        /// </summary>
        public void EndConversation()
        {
            IsInConversation = false;
            Stt?.StopRecording();
            Tts?.Stop();
            Emotion?.ReleaseEmotion();
            Debug.Log("[SPS Conv] Conversation ended");
        }

        /// <summary>
        /// Called when STT produces a transcription.
        /// </summary>
        private void OnTranscriptionReceived(string text)
        {
            if (string.IsNullOrEmpty(text) || !IsInConversation) return;

            _lastUserMessage = text;
            Debug.Log($"[SPS Conv] User: {text}");

            // React emotionally to the user's message.
            Emotion?.ReactToMessage(text);

            // Send to SPS kernel for LLM response.
            StartCoroutine(SendToKernelCoroutine(text));
        }

        /// <summary>
        /// Send the user's message to the SPS kernel and get a response.
        /// </summary>
        private IEnumerator SendToKernelCoroutine(string userMessage)
        {
            Emotion?.SetThinking();

            var payload = new ConversationRequest
            {
                message = userMessage,
                context = _lastAssistantMessage
            };
            string json = JsonUtility.ToJson(payload);

            using (UnityWebRequest request = new UnityWebRequest(
                $"{SpsServerUrl}/api/llm/complete", "POST"))
            {
                byte[] bodyRaw = Encoding.UTF8.GetBytes(json);
                request.uploadHandler = new UploadHandlerRaw(bodyRaw);
                request.downloadHandler = new DownloadHandlerBuffer();
                request.SetRequestHeader("Content-Type", "application/json");

                yield return request.SendWebRequest();

                if (request.result == UnityWebRequest.Result.Success)
                {
                    string responseText = request.downloadHandler.text;
                    var response = JsonUtility.FromJson<ConversationResponse>(responseText);
                    string assistantMessage = response?.text ?? "عذراً، لم أتمكن من فهم ذلك.";

                    _lastAssistantMessage = assistantMessage;
                    Debug.Log($"[SPS Conv] Assistant: {assistantMessage}");

                    // Speak the response via TTS.
                    Tts?.Speak(assistantMessage);

                    // Set speaking emotion.
                    Emotion?.LockEmotion(CompanionEmotion.Speaking);

                    // Auto-listen after speaking if enabled.
                    if (AutoListenAfterSpeak && IsInConversation)
                    {
                        // Wait for TTS to finish, then start listening again.
                        StartCoroutine(AutoListenAfterTts());
                    }
                }
                else
                {
                    Debug.LogWarning($"[SPS Conv] Kernel request failed: {request.error}");
                    Emotion?.SetEmotion(CompanionEmotion.Sad, 3f);
                }
            }
        }

        private IEnumerator AutoListenAfterTts()
        {
            // Wait for TTS to finish.
            while (Tts != null && Tts.IsSpeaking)
            {
                yield return null;
            }

            // Small pause before listening again.
            yield return new WaitForSeconds(0.5f);

            if (IsInConversation)
            {
                Stt?.StartRecording();
            }
        }

        /// <summary>
        /// Process a text message directly (no voice).
        /// </summary>
        public void ProcessTextMessage(string message)
        {
            if (!IsInConversation) IsInConversation = true;
            OnTranscriptionReceived(message);
        }

        [Serializable]
        private class ConversationRequest
        {
            public string message;
            public string context;
        }

        [Serializable]
        private class ConversationResponse
        {
            public string text;
        }
    }
}
