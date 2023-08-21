lint:
	cargo clippy --all-targets -- -D warnings

schema:
	cargo schema

ts-codegen:
	npm install -g @cosmwasm/ts-codegen

types: schema
	cosmwasm-ts-codegen generate \
		--plugin client \
		--schema ./schema \
		--out ./ts \
		--name FrenParty \
		--no-bundle
