#!/bin/bash
#

if [ -x ../../target/release/edc2svd ]; then
    EDC2SVD=../../target/release/edc2svd
elif  [ -x ../../target/debug/edc2svd ]; then
    EDC2SVD=../../target/debug/edc2svd
else
    echo "cannot find edc2svd"
    exit -1
fi

test_edc() {
    local edc=$1
    local svd=$2
    shift 2
    local args=$*
    temp=$(mktemp --suffix .svd)
    echo "executing $EDC2SVD $args $edc $temp"
    $EDC2SVD $args $edc $temp
    diff -us $svd $temp
    result=$?
    #rm -f $temp
    if [ $result -ne 0 ]; then
        echo "aborting test script"
        exit -1
    fi
}

test_edc PIC32MX170F256B.PIC PIC32MX170F256B.svd
test_edc PIC32MX274F256B.PIC PIC32MX274F256B.svd
test_edc PIC32MX470F512H.PIC PIC32MX470F512H.svd

#end
