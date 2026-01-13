#!/usr/bin/env bash

set -e

DIR=`dirname $0`

VERSION=`$DIR/version.sh`

#
# Define ARCH
#
# if [ -z "${!ARCH}" ]; then
if [ -z "$(eval echo \$ARCH)" ]; then
  echo "Error: ARCH is not defined or is empty."
  exit 1
fi

#
# Define OS_TYPE
#
if [[ "$(uname)" == "Darwin" ]]; then
  OS_TYPE="darwin"
elif [[ "$(uname)" == "Linux" ]]; then
  OS_TYPE="linux"
else
  echo "Error: Unsupported operating system."
  exit 1
fi

#
# Define TRIPLE and OS_ARCH
#
case "$OS_TYPE" in
  linux)
    TRIPLE="$ARCH-unknown-linux-musl"
    OS_ARCH="linux-$ARCH-musl"
    ;;
  darwin)
    TRIPLE="$ARCH-apple-darwin"
    OS_ARCH="darwin-$ARCH"
    ;;
  *)
    echo "Error: Unsupported OS_TYPE ($OS_TYPE)."
    exit 1
    ;;
esac

mkdir -p release/terminai-$VERSION-$OS_ARCH

cargo build -p termin --release --target=$TRIPLE

cp target/$TRIPLE/release/terminai release/terminai-$VERSION-$OS_ARCH/terminai

# Copy Python agent
cp -r python release/terminai-$VERSION-$OS_ARCH/

tar -czvf release/terminai-$VERSION-$OS_ARCH.tar.gz \
  -C release/terminai-$VERSION-$OS_ARCH \
  terminai python
