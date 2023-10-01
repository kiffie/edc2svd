#!/bin/bash
#

if [ -x ../../target/release/edc2svd ]; then
    EDC2SVD=../../target/release/edc2svd
elif  [ -x ../../target/debug/edc2svd ]; then
    EDC2SVD=../../target/debug/edc2svd
else
    echo "cannot find edc2svd"
    exit
fi

test_edc() {
    local edc=$1.PIC
    local svd=$1.svd
    local lib=$1.lib.rs
    shift 1
    local args=$*
    temp=$(mktemp --suffix .svd)
    echo "executing $EDC2SVD $args $edc $temp"
    $EDC2SVD $args $edc $temp
    diff -us $svd $temp || exit
    #rm -f $temp
    echo "executing svd2rust --target mips -i $svd"
    svd2rust --target mips -i $svd
    diff -us $lib lib.rs || exit
}

test_edc PIC32MX170F256B
test_edc PIC32MX274F256B
test_edc PIC32MX470F512H
test_edc PIC32MX695F512L

#end
