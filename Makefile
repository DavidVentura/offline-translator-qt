# Cargo.toml is single source of truth for the version
# I didn't want to generate packaging/ubuntu-touch/manifest.json
# so that's checked instead

VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
DIST := dist
TAG := v$(VERSION)

APPIMAGE := packaging/appimage/out/Offline_translator-$(VERSION)-x86_64.AppImage
CLICK_ARM64 := build/aarch64-linux-gnu/app/dev.davidv.translator_$(VERSION)_arm64.click
CLICK_AMD64 := build/x86_64-linux-gnu/app/dev.davidv.translator_$(VERSION)_amd64.click
WINZIP := target/windows-deploy.zip

.PHONY: release check-version appimage click-arm64 click-amd64 windows dist clean-dist

release: dist
	@echo
	@echo "Artifacts in $(DIST)/:"
	@ls -1sh $(DIST)/
	@echo
	@echo "Publish with:"
	@echo
	@echo "  git tag -a $(TAG) -m '$(TAG)' && git push origin $(TAG)"
	@echo "  gh release create $(TAG) \\"
	@for f in $(DIST)/*; do echo "      $$f \\"; done
	@echo "      --title '$(TAG)' --notes 'TODO'"
	@echo

# Guards the one duplicated version (packaging/ubuntu-touch/manifest.json).
check-version:
	@./check_vers_match.sh
	@echo "version $(VERSION)"

appimage: check-version
	./packaging/appimage/build-appimage.sh

# Ubuntu Touch. The clickable arch names differ from the build triplet directories.
click-arm64: check-version
	./clickable/package-click.sh -a arm64

click-amd64: check-version
	./clickable/package-click.sh -a amd64

# deploy.sh builds, assembles the tree and writes the zip.
windows: check-version
	. ./packaging/windows/env.sh && ./packaging/windows/deploy.sh

dist: appimage click-arm64 click-amd64 windows
	@rm -rf $(DIST) && mkdir -p $(DIST)
	@cp $(APPIMAGE) $(DIST)/
	@cp $(CLICK_ARM64) $(DIST)/
	@cp $(CLICK_AMD64) $(DIST)/
	@cp $(WINZIP) $(DIST)/offline-translator-$(VERSION)-windows-x64.zip
	@[ -f $(APPIMAGE).zsync ] && cp $(APPIMAGE).zsync $(DIST)/ || true

clean-dist:
	rm -rf $(DIST)
