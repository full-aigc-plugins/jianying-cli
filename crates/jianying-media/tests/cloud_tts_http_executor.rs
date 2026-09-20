use jianying_media::{
    AliyunBailianTtsAdapter, AliyunTtsFamily, BaiduTtsAdapter, CloudTtsHttpExecutor,
    CloudTtsProviderAdapter, CredentialRef, CredentialSource, MiniMaxTtsAdapter,
    TencentCloudTtsAdapter, TtsAudioFormat, TtsRequest, VolcengineTtsAdapter, XiaomiMimoTtsAdapter,
    ZhipuGlmTtsAdapter,
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

type MockServer = (
    String,
    Arc<Mutex<Vec<Vec<u8>>>>,
    std::thread::JoinHandle<()>,
);

#[test]
fn all_cloud_http_adapters_execute_against_loopback_without_leaking_protocol_shape() {
    run_json_case(
        "XIAOMI_MIMO_HTTP_EXECUTOR_TEST_KEY",
        "xiaomi-secret",
        CloudTtsProviderAdapter::XiaomiMimo(XiaomiMimoTtsAdapter::new().unwrap()),
        request(TtsAudioFormat::Wav, "mimo-v2.5-tts", "mimo-voice"),
        br#"{"id":"xiaomi-42","choices":[{"message":{"audio":{"data":"UklGRmZha2U="}}}]}"#,
        "authorization: bearer xiaomi-secret",
        b"RIFFfake",
    );
    run_json_case(
        "MINIMAX_HTTP_EXECUTOR_TEST_KEY",
        "minimax-secret",
        CloudTtsProviderAdapter::MiniMax(MiniMaxTtsAdapter::new().unwrap()),
        request(TtsAudioFormat::Mp3, "speech-2.8-hd", "male-qn-qingse"),
        br#"{"data":{"audio":"49443366616b65"},"trace_id":"minimax-42","base_resp":{"status_code":0,"status_msg":"success"}}"#,
        "authorization: bearer minimax-secret",
        b"ID3fake",
    );
    run_json_case(
        "TENCENT_HTTP_EXECUTOR_TEST_KEY",
        r#"{"secret_id":"tencent-id","secret_key":"tencent-secret"}"#,
        CloudTtsProviderAdapter::TencentCloud(TencentCloudTtsAdapter::new().unwrap()),
        request(TtsAudioFormat::Wav, "1", "101001"),
        br#"{"Response":{"Audio":"UklGRmZha2U=","RequestId":"tencent-42"}}"#,
        "x-tc-action: texttovoice",
        b"RIFFfake",
    );
}

#[test]
fn volcengine_v3_executes_sse_with_api_headers_and_concatenates_audio_frames() {
    let adapter = CloudTtsProviderAdapter::Volcengine(
        VolcengineTtsAdapter::new("seed-tts-2.0", "user-1").unwrap(),
    );
    let request = credentialed_request(
        request(TtsAudioFormat::Wav, "", "zh_female_fixture"),
        "VOLC_HTTP_EXECUTOR_TEST_KEY",
        "volc-secret",
    );
    let response_body = b"data: {\"code\":0,\"data\":\"UklG\"}\n\ndata: {\"code\":0,\"data\":\"RmZha2U=\"}\n\ndata: {\"code\":20000000,\"message\":\"done\"}\n\n";
    let (endpoint, requests, handle) = serve(vec![http_response(
        "200 OK",
        "text/event-stream",
        response_body,
        &[(&"X-Api-Request-Id", &"volc-response-42")],
    )]);
    let output = temporary_path("volc-v3.wav");
    let artifact = CloudTtsHttpExecutor::with_loopback_endpoint(endpoint, Duration::from_secs(2))
        .unwrap()
        .execute(&adapter, &request, &output)
        .unwrap();
    handle.join().unwrap();

    assert_eq!(artifact.request_id(), Some("volc-response-42"));
    assert_eq!(std::fs::read(output).unwrap(), b"RIFFfake");
    let request_text = String::from_utf8_lossy(&requests.lock().unwrap()[0]).to_ascii_lowercase();
    assert!(request_text.contains("x-api-key: volc-secret"));
    assert!(request_text.contains("x-api-resource-id: seed-tts-2.0"));
    assert!(request_text.contains("x-api-request-id:"));
    assert!(request_text.contains("accept: text/event-stream"));
    assert!(!request_text.contains("authorization:"));
}

#[test]
fn binary_form_and_remote_url_providers_execute_with_expected_auth() {
    let baidu = CloudTtsProviderAdapter::Baidu(BaiduTtsAdapter::new("device-1").unwrap());
    let baidu_request = credentialed_request(
        request(TtsAudioFormat::Mp3, "", "0"),
        "BAIDU_HTTP_EXECUTOR_TEST_KEY",
        "baidu-token",
    );
    let (endpoint, requests, handle) = serve(vec![http_response(
        "200 OK",
        "audio/mpeg",
        b"ID3baidu",
        &[(&"X-Request-Id", &"baidu-42")],
    )]);
    let output = temporary_path("baidu.mp3");
    CloudTtsHttpExecutor::with_loopback_endpoint(endpoint, Duration::from_secs(2))
        .unwrap()
        .execute(&baidu, &baidu_request, &output)
        .unwrap();
    handle.join().unwrap();
    let request_bytes = requests.lock().unwrap()[0].clone();
    let request_text = String::from_utf8_lossy(&request_bytes);
    assert!(request_text.contains("tok=baidu-token"));
    assert_eq!(std::fs::read(&output).unwrap(), b"ID3baidu");

    let zhipu = CloudTtsProviderAdapter::ZhipuGlm(ZhipuGlmTtsAdapter::new().unwrap());
    let zhipu_request = credentialed_request(
        request(TtsAudioFormat::Wav, "glm-tts", "tongtong"),
        "ZHIPU_HTTP_EXECUTOR_TEST_KEY",
        "zhipu-secret",
    );
    let (endpoint, requests, handle) = serve(vec![http_response(
        "200 OK",
        "audio/wav",
        b"RIFFzhipu",
        &[],
    )]);
    let output = temporary_path("zhipu.wav");
    CloudTtsHttpExecutor::with_loopback_endpoint(endpoint, Duration::from_secs(2))
        .unwrap()
        .execute(&zhipu, &zhipu_request, &output)
        .unwrap();
    handle.join().unwrap();
    assert!(String::from_utf8_lossy(&requests.lock().unwrap()[0])
        .to_ascii_lowercase()
        .contains("authorization: bearer zhipu-secret"));
    assert_eq!(std::fs::read(&output).unwrap(), b"RIFFzhipu");

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let endpoint = format!("http://{address}/tts");
    let audio_url = format!("http://{address}/audio");
    let body = format!(
        r#"{{"request_id":"aliyun-42","output":{{"audio":{{"url":"{audio_url}","expires_at":"2099-01-01T00:00:00Z"}}}}}}"#
    );
    let requests = Arc::new(Mutex::new(Vec::new()));
    let handle = spawn_server(
        listener,
        vec![
            http_response("200 OK", "application/json", body.as_bytes(), &[]),
            http_response("200 OK", "audio/wav", b"RIFFaliyun", &[]),
        ],
        requests.clone(),
    );
    std::env::set_var("ALIYUN_HTTP_EXECUTOR_TEST_KEY", "aliyun-secret");
    let aliyun_request = TtsRequest::new("阿里语音", TtsAudioFormat::Wav)
        .unwrap()
        .with_model("qwen-audio-3.1-tts-flash")
        .with_voice("longanhuan_v3.6")
        .with_credential(
            CredentialRef::new(
                CredentialSource::Environment,
                "ALIYUN_HTTP_EXECUTOR_TEST_KEY",
            )
            .unwrap(),
        );
    let aliyun = CloudTtsProviderAdapter::AliyunBailian(
        AliyunBailianTtsAdapter::new(AliyunTtsFamily::QwenAudio, Some("workspace-1")).unwrap(),
    );
    let output = temporary_path("aliyun.wav");
    CloudTtsHttpExecutor::with_loopback_endpoint(endpoint, Duration::from_secs(2))
        .unwrap()
        .execute(&aliyun, &aliyun_request, &output)
        .unwrap();
    handle.join().unwrap();
    assert_eq!(requests.lock().unwrap().len(), 2);
    assert_eq!(std::fs::read(&output).unwrap(), b"RIFFaliyun");
}

#[test]
fn credential_and_endpoint_fail_closed_before_network_submission() {
    assert!(CloudTtsHttpExecutor::with_loopback_endpoint(
        "https://example.com/tts",
        Duration::from_secs(1)
    )
    .is_err());
    let missing_name = "JY_TTS_EXECUTOR_INTENTIONALLY_MISSING_KEY";
    std::env::remove_var(missing_name);
    let request = TtsRequest::new("拒绝", TtsAudioFormat::Mp3)
        .unwrap()
        .with_model("speech-2.8-hd")
        .with_voice("male-qn-qingse")
        .with_credential(CredentialRef::new(CredentialSource::Environment, missing_name).unwrap());
    let error = CloudTtsHttpExecutor::with_loopback_endpoint(
        "http://127.0.0.1:9/tts",
        Duration::from_millis(100),
    )
    .unwrap()
    .execute(
        &CloudTtsProviderAdapter::MiniMax(MiniMaxTtsAdapter::new().unwrap()),
        &request,
        &temporary_path("missing.mp3"),
    )
    .unwrap_err();
    assert!(!error.is_ambiguous());
    assert!(error.to_string().contains(missing_name));
}

fn run_json_case(
    env_name: &str,
    secret: &str,
    adapter: CloudTtsProviderAdapter,
    request: TtsRequest,
    response_body: &[u8],
    expected_header: &str,
    expected_audio: &[u8],
) {
    let request = credentialed_request(request, env_name, secret);
    let (endpoint, requests, handle) = serve(vec![http_response(
        "200 OK",
        "application/json",
        response_body,
        &[],
    )]);
    let output = temporary_path(&format!("{}.audio", adapter.provider_id()));
    let artifact = CloudTtsHttpExecutor::with_loopback_endpoint(endpoint, Duration::from_secs(2))
        .unwrap()
        .execute(&adapter, &request, &output)
        .unwrap();
    handle.join().unwrap();
    assert_eq!(artifact.provider_id(), adapter.provider_id());
    assert_eq!(std::fs::read(output).unwrap(), expected_audio);
    assert!(String::from_utf8_lossy(&requests.lock().unwrap()[0])
        .to_ascii_lowercase()
        .contains(expected_header));
}

fn request(format: TtsAudioFormat, model: &str, voice: &str) -> TtsRequest {
    let mut request = TtsRequest::new("离线执行合约", format).unwrap();
    if !model.is_empty() {
        request = request.with_model(model);
    }
    if !voice.is_empty() {
        request = request.with_voice(voice);
    }
    request
}

fn credentialed_request(request: TtsRequest, env_name: &str, secret: &str) -> TtsRequest {
    std::env::set_var(env_name, secret);
    request.with_credential(CredentialRef::new(CredentialSource::Environment, env_name).unwrap())
}

fn serve(responses: Vec<Vec<u8>>) -> MockServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/tts", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let handle = spawn_server(listener, responses, requests.clone());
    (endpoint, requests, handle)
}

fn spawn_server(
    listener: TcpListener,
    responses: Vec<Vec<u8>>,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_http_request(&mut stream);
            requests.lock().unwrap().push(request);
            stream.write_all(&response).unwrap();
        }
    })
}

fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stream.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            let header_end = header_end + 4;
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("Content-Length: ")
                        .or_else(|| line.strip_prefix("content-length: "))
                })
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if request.len() >= header_end + content_length {
                break;
            }
        }
    }
    request
}

fn http_response(
    status: &str,
    content_type: &str,
    body: &[u8],
    extra_headers: &[(&&str, &&str)],
) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in extra_headers {
        response.push_str(name);
        response.push_str(": ");
        response.push_str(value);
        response.push_str("\r\n");
    }
    response.push_str("\r\n");
    let mut bytes = response.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

fn temporary_path(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "jianying-cloud-tts-{}-{}",
        std::process::id(),
        name.replace('/', "-")
    ));
    let _ = std::fs::remove_file(&root);
    root
}
