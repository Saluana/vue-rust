trace('loose', null == undefined, null == 0, undefined == 0, '' == 0, '0' == 0, false == '', true == 1)
trace('strict', null === undefined, 1 === '1', NaN === NaN, 'a' === 'a', -0 === 0)
trace('not', 1 != '1', 1 !== '1', null != undefined, 0 != false)
