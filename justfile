# vllm serve /shelf/models/Qwen2.5-72B-Instruct-AWQ --host 127.0.0.1 --port 8081 --served-model-name qwen --max-model-len 9400 --max-num-seqs 256 --quantization awq_marlin --swap-space 8 -pp 3
up:
    CUDA_VISIBLE_DEVICES="0" nohup ./bin/flow-judge.llamafile -c 8192 -ngl 32 --temp 0.1 -n 1000 --host 127.0.0.1 --port 8080 -t 16 --nobrowser --server --cont-batching &> /dev/null & disown;\
    CUDA_VISIBLE_DEVICES="1,2" nohup llama-server -ngl 81 -t 16 -m '/shelf/models/Qwen2.5-72B-Instruct-IQ4_XS.gguf' -c 8192 -n 8192 --host 127.0.0.1 --port 8081 --temp 0.1 --chat-template chatml --cont-batching &> /dev/null & disown

down:
    pkill -f flow-judge.llamafile
    pkill -f llama-server

eval:
    trap 'just down' EXIT; just up; sleep 10; cargo run
