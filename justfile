lint:
	cargo clippy --all-targets -- -D warnings

schema:
	cargo schema
