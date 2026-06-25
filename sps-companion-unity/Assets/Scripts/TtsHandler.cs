// SPS Emotional Companion — TTS Handler
// Phase 14: Text-to-Speech via SPS kernel (Coqui/ElevenLabs)

using System;
using System.Collections;
using System.Text;
using UnityEngine;
using UnityEngine.Networking;

namespace SPS.Companion
{
    /// <summary>
    /// Handles TTS requests to the SPS kernel.
    /// The kernel generates audio via Coqui/ElevenLabs and returns
    /// base64-encoded WAV data.
    /// </summary>
    public class TtsHandler : MonoBehaviour
    {
        [Header("SPS Connection")]
        public string SpsServerUrl = "http://localhost:7780";

        [Header("Audio")]
        public AudioSource AudioSource;
        public float DefaultVolume = 1.0f;

        [Header("Voice Settings")]
        public string DefaultVoice = "ar-AR-Hamed";
        public float SpeechRate = 1.0f;
        public float Pitch = 1.0f;

        private bool _isSpeaking = false;

        /// <summary>
        /// Speak the given text via TTS.
        /// </summary>
        public void Speak(string text)
        {
            if (_isSpeaking) return;
            StartCoroutine(SpeakCoroutine(text));
        }

        /// <summary>
        /// Stop current TTS playback.
        /// </summary>
        public void Stop()
        {
            if (AudioSource != null && AudioSource.isPlaying)
            {
                AudioSource.Stop();
            }
            _isSpeaking = false;
        }

        /// <summary>
        /// Is TTS currently playing?
        /// </summary>
        public bool IsSpeaking => _isSpeaking;

        private IEnumerator SpeakCoroutine(string text)
        {
            _isSpeaking = true;

            // Build TTS request payload.
            var payload = new TtsRequest
            {
                text = text,
                voice = DefaultVoice,
                rate = SpeechRate,
                pitch = Pitch
            };
            string json = JsonUtility.ToJson(payload);

            // Send request to SPS kernel.
            using (UnityWebRequest request = new UnityWebRequest(
                $"{SpsServerUrl}/api/tts/generate", "POST"))
            {
                byte[] bodyRaw = Encoding.UTF8.GetBytes(json);
                request.uploadHandler = new UploadHandlerRaw(bodyRaw);
                request.downloadHandler = new DownloadHandlerAudioClip(
                    $"{SpsServerUrl}/api/tts/generate", AudioType.WAV);
                request.SetRequestHeader("Content-Type", "application/json");

                yield return request.SendWebRequest();

                if (request.result == UnityWebRequest.Result.Success)
                {
                    AudioClip clip = DownloadHandlerAudioClip.GetContent(request);
                    if (clip != null && AudioSource != null)
                    {
                        AudioSource.clip = clip;
                        AudioSource.volume = DefaultVolume;
                        AudioSource.Play();
                        Debug.Log($"[SPS TTS] Playing: {text.Substring(0, Math.Min(50, text.Length))}...");

                        // Wait for audio to finish.
                        yield return new WaitForSeconds(clip.length);
                    }
                }
                else
                {
                    Debug.LogWarning($"[SPS TTS] Request failed: {request.error}");
                }
            }

            _isSpeaking = false;
        }

        [Serializable]
        private class TtsRequest
        {
            public string text;
            public string voice;
            public float rate;
            public float pitch;
        }
    }
}
