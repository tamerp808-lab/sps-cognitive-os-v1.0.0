// SPS Emotional Companion — STT Handler
// Phase 14: Speech-to-Text via microphone + SPS kernel (Coqui/Whisper)

using System;
using System.Collections;
using System.Text;
using UnityEngine;
using UnityEngine.Networking;

namespace SPS.Companion
{
    /// <summary>
    /// Handles microphone recording + STT via SPS kernel.
    /// Records audio → sends to kernel → receives text transcription.
    /// </summary>
    [RequireComponent(typeof(AudioSource))]
    public class SttHandler : MonoBehaviour
    {
        [Header("SPS Connection")]
        public string SpsServerUrl = "http://localhost:7780";

        [Header("Recording Settings")]
        public int SampleRate = 16000;
        public float MaxRecordingDuration = 30f;
        public float SilenceThreshold = 0.01f;
        public float SilenceDuration = 2f;

        [Header("State")]
        public bool IsRecording = false;

        private AudioSource _audioSource;
        private AudioClip _recordedClip;
        private float _recordingTimer = 0f;
        private float _silenceTimer = 0f;
        private bool _autoStopOnSilence = true;

        public event Action<string> OnTranscriptionReceived;
        public event Action OnRecordingStarted;
        public event Action OnRecordingStopped;

        void Start()
        {
            _audioSource = GetComponent<AudioSource>();
        }

        /// <summary>
        /// Start recording from microphone.
        /// </summary>
        public void StartRecording(bool autoStopOnSilence = true)
        {
            if (IsRecording) return;

            _autoStopOnSilence = autoStopOnSilence;
            _recordingTimer = 0f;
            _silenceTimer = 0f;

            _recordedClip = Microphone.Start(null, false, (int)MaxRecordingDuration, SampleRate);
            IsRecording = true;

            OnRecordingStarted?.Invoke();
            Debug.Log("[SPS STT] Recording started");
        }

        /// <summary>
        /// Stop recording and transcribe.
        /// </summary>
        public void StopRecording()
        {
            if (!IsRecording) return;

            Microphone.End(null);
            IsRecording = false;
            OnRecordingStopped?.Invoke();
            Debug.Log("[SPS STT] Recording stopped, transcribing...");

            StartCoroutine(TranscribeCoroutine());
        }

        void Update()
        {
            if (!IsRecording) return;

            _recordingTimer += Time.deltaTime;

            // Check for silence (auto-stop).
            if (_autoStopOnSilence && _recordingTimer > 1f)
            {
                float level = GetAudioLevel();
                if (level < SilenceThreshold)
                {
                    _silenceTimer += Time.deltaTime;
                    if (_silenceTimer >= SilenceDuration)
                    {
                        StopRecording();
                    }
                }
                else
                {
                    _silenceTimer = 0f;
                }
            }

            // Max duration reached.
            if (_recordingTimer >= MaxRecordingDuration)
            {
                StopRecording();
            }
        }

        private float GetAudioLevel()
        {
            if (_recordedClip == null) return 0f;

            int samples = 256;
            float[] data = new float[samples];
            int micPosition = Microphone.GetPosition(null) - samples;
            if (micPosition < 0) return 0f;

            _recordedClip.GetData(data, micPosition);

            float sum = 0f;
            for (int i = 0; i < samples; i++)
            {
                sum += Mathf.Abs(data[i]);
            }
            return sum / samples;
        }

        private IEnumerator TranscribeCoroutine()
        {
            if (_recordedClip == null) yield break;

            // Convert AudioClip to WAV bytes.
            byte[] wavBytes = AudioClipToWav(_recordedClip);
            string base64 = Convert.ToBase64String(wavBytes);

            // Send to SPS kernel for transcription.
            var payload = new SttRequest { audio_base64 = base64, sample_rate = SampleRate };
            string json = JsonUtility.ToJson(payload);

            using (UnityWebRequest request = new UnityWebRequest(
                $"{SpsServerUrl}/api/stt/transcribe", "POST"))
            {
                byte[] bodyRaw = Encoding.UTF8.GetBytes(json);
                request.uploadHandler = new UploadHandlerRaw(bodyRaw);
                request.downloadHandler = new DownloadHandlerBuffer();
                request.SetRequestHeader("Content-Type", "application/json");

                yield return request.SendWebRequest();

                if (request.result == UnityWebRequest.Result.Success)
                {
                    string responseText = request.downloadHandler.text;
                    var response = JsonUtility.FromJson<SttResponse>(responseText);
                    string transcription = response?.text ?? "";

                    Debug.Log($"[SPS STT] Transcribed: {transcription}");
                    OnTranscriptionReceived?.Invoke(transcription);
                }
                else
                {
                    Debug.LogWarning($"[SPS STT] Transcription failed: {request.error}");
                    OnTranscriptionReceived?.Invoke("");
                }
            }
        }

        private byte[] AudioClipToWav(AudioClip clip)
        {
            float[] samples = new float[clip.samples * clip.channels];
            clip.GetData(samples, 0);

            byte[] wavData = new byte[44 + samples.Length * 2];
            int freq = clip.frequency;
            int channels = clip.channels;

            // WAV header.
            BitConverter.GetBytes(0x46464952).CopyTo(wavData, 0);   // "RIFF"
            BitConverter.GetBytes(wavData.Length - 8).CopyTo(wavData, 4);
            BitConverter.GetBytes(0x45564157).CopyTo(wavData, 8);   // "WAVE"
            BitConverter.GetBytes(0x20746d66).CopyTo(wavData, 12);  // "fmt "
            BitConverter.GetBytes(16).CopyTo(wavData, 16);
            BitConverter.GetBytes((short)1).CopyTo(wavData, 20);    // PCM
            BitConverter.GetBytes((short)channels).CopyTo(wavData, 22);
            BitConverter.GetBytes(freq).CopyTo(wavData, 24);
            BitConverter.GetBytes(freq * channels * 2).CopyTo(wavData, 28);
            BitConverter.GetBytes((short)(channels * 2)).CopyTo(wavData, 32);
            BitConverter.GetBytes((short)16).CopyTo(wavData, 34);
            BitConverter.GetBytes(0x61746164).CopyTo(wavData, 36);  // "data"
            BitConverter.GetBytes(samples.Length * 2).CopyTo(wavData, 40);

            // PCM data.
            for (int i = 0; i < samples.Length; i++)
            {
                short val = (short)(samples[i] * 32767f);
                BitConverter.GetBytes(val).CopyTo(wavData, 44 + i * 2);
            }

            return wavData;
        }

        [Serializable]
        private class SttRequest
        {
            public string audio_base64;
            public int sample_rate;
        }

        [Serializable]
        private class SttResponse
        {
            public string text;
        }
    }
}
