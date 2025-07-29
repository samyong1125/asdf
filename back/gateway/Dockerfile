FROM envoyproxy/envoy:v1.34.4

# Envoy 설정 파일 복사
COPY envoy.yaml /etc/envoy/envoy.yaml

# 포트 노출
EXPOSE 15000 9901

# Envoy 실행
CMD ["/usr/local/bin/envoy", "-c", "/etc/envoy/envoy.yaml"]