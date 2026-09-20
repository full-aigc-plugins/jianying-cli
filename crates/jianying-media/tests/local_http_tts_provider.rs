use jianying_media::{LocalHttpTtsProvider, TtsAudioFormat, TtsProvider, TtsRequest};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

#[test]
fn local_http_rejects_non_loopback_endpoints() {
    assert!(
        LocalHttpTtsProvider::new("http://example.com/tts", TtsAudioFormat::Wav)
            .expect_err("本地 Provider 不得访问非回环地址")
            .to_string()
            .contains("loopback")
    );
    assert!(
        LocalHttpTtsProvider::new("https://127.0.0.1/tts", TtsAudioFormat::Wav)
            .expect_err("本地 Provider 只接受显式本机 HTTP")
            .to_string()
            .contains("http://")
    );
}

#[test]
fn local_http_posts_unified_json_and_persists_binary_audio() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 4096];
        loop {
            let count = stream.read(&mut chunk).unwrap();
            assert!(count > 0, "请求头不完整");
            buffer.extend_from_slice(&chunk[..count]);
            if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let header_end = buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap()
            + 4;
        let headers = String::from_utf8_lossy(&buffer[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(str::trim)
                    .and_then(|value| value.parse::<usize>().ok())
            })
            .unwrap();
        while buffer.len() - header_end < content_length {
            let count = stream.read(&mut chunk).unwrap();
            assert!(count > 0, "请求体不完整");
            buffer.extend_from_slice(&chunk[..count]);
        }
        let body = &buffer[header_end..header_end + content_length];
        assert!(String::from_utf8_lossy(body).contains("本地模型"));
        let audio = b"RIFFfixture-wave-bytes";
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nX-Request-Id: local-42\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            audio.len()
        )
        .unwrap();
        stream.write_all(audio).unwrap();
    });
    let root = std::env::temp_dir().join(format!("jianying-local-http-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let output = root.join("voice.wav");
    let provider =
        LocalHttpTtsProvider::new(format!("http://{address}/tts"), TtsAudioFormat::Wav).unwrap();
    let request = TtsRequest::new("本地模型", TtsAudioFormat::Wav).unwrap();

    let artifact = provider.synthesize(&request, &output).unwrap();

    server.join().unwrap();
    assert_eq!(artifact.provider_id(), "local-http");
    assert_eq!(artifact.request_id(), Some("local-42"));
    assert_eq!(std::fs::read(output).unwrap(), b"RIFFfixture-wave-bytes");
    let _ = std::fs::remove_dir_all(root);
}
