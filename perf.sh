#cargo b -r && perf record --call-graph=lbr target/release/deproject-ui

RUSTFLAGS='-C target-cpu=native' cargo b -r && perf record --call-graph=lbr target/release/deproject-ui
