################################################################################
#
# slowos
#
################################################################################

SLOWOS_VERSION = 0.1.0
SLOWOS_SITE = $(BR2_EXTERNAL_SLOWOS_PATH)/..
SLOWOS_SITE_METHOD = local
SLOWOS_LICENSE = MIT
SLOWOS_LICENSE_FILES = LICENSE

# All the binaries we build
SLOWOS_BINARIES = \
	slowdesktop \
	slowwrite \
	slowpaint \
	slowbooks \
	slowsheets \
	slownotes \
	slowchess \
	files \
	slowmusic \
	slowslides \
	slowtex \
	trash \
	slowterm \
	slowpics

define SLOWOS_BUILD_CMDS
	# Set up Rust cross-compilation environment
	export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=$(TARGET_CC); \
	export CC_aarch64_unknown_linux_gnu=$(TARGET_CC); \
	export PKG_CONFIG_PATH=$(STAGING_DIR)/usr/lib/pkgconfig; \
	export PKG_CONFIG_SYSROOT_DIR=$(STAGING_DIR); \
	cd $(@D) && \
	$(HOST_DIR)/bin/cargo build \
		--release \
		--target aarch64-unknown-linux-gnu \
		$(foreach bin,$(SLOWOS_BINARIES),-p $(bin))
endef

define SLOWOS_INSTALL_TARGET_CMDS
	$(foreach bin,$(SLOWOS_BINARIES), \
		$(INSTALL) -D -m 0755 \
			$(@D)/target/aarch64-unknown-linux-gnu/release/$(bin) \
			$(TARGET_DIR)/usr/bin/$(bin); \
	)
	# Install desktop shell as the default session
	$(INSTALL) -D -m 0755 \
		$(@D)/target/aarch64-unknown-linux-gnu/release/slowdesktop \
		$(TARGET_DIR)/usr/bin/slowdesktop
endef

$(eval $(generic-package))
