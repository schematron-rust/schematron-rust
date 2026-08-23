#!/bin/sh
# Fetches the ISO Schematron reference implementation, for differential
# testing against it.
#
# The stylesheets are third-party and are deliberately NOT vendored into this
# repository: they carry their own licence and their own release cadence, and
# a copy here would rot. Fetch them into a directory of your choosing and
# point SCHEMATRON_SKELETON at it:
#
#   sh tests/differential/fetch-skeleton.sh /tmp/skeleton
#   SCHEMATRON_SKELETON=/tmp/skeleton cargo test --test differential -- --ignored
#
# Requires curl and xsltproc.
set -eu

target="${1:-}"
if [ -z "$target" ]; then
    echo "usage: $0 <directory>" >&2
    exit 2
fi
mkdir -p "$target"

base="https://raw.githubusercontent.com/Schematron/schematron/master/trunk/schematron/code"
for file in \
    iso_dsdl_include.xsl \
    iso_abstract_expand.xsl \
    iso_svrl_for_xslt1.xsl \
    iso_schematron_skeleton_for_xslt1.xsl
do
    printf '%-42s' "$file"
    curl -sfL "$base/$file" -o "$target/$file"
    echo "ok"
done

echo
echo "Now run:"
echo "  SCHEMATRON_SKELETON=$target cargo test --test differential -- --ignored"
