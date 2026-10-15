import test from 'node:test';
import assert from 'node:assert/strict';
import {reading, formatChartValue, formatBucketTime} from '../static/chart-interactions.js';

test('missing readings stay different from a true zero', () => {
    for (const value of [undefined, null, '', ' ', 'not a number', Infinity]) {
        assert.equal(reading(value), null);
        assert.equal(formatChartValue(value), 'No readings');
    }
    assert.equal(reading('0'), 0);
    assert.equal(formatChartValue('0'), '0.0');
    assert.equal(formatChartValue('11143.695'), '11,143.7');
    assert.equal(formatChartValue('-6.523'), '-6.52');
});

test('small nonzero measurements are not rounded to a false zero', () => {
    assert.equal(formatChartValue('0.00003215'), '0.0000322');
    assert.equal(formatChartValue('-0.00003215'), '-0.0000322');
});

test('bucket detail preserves recorded wall time and identifies midnight rollover', () => {
    assert.equal(formatBucketTime('2026-09-30 23:00:00','hour'), '30 Sep 2026 · 23:00–00:00 (+1 day)');
    assert.equal(formatBucketTime('2026-09-30 00:00:00','hour'), '30 Sep 2026 · 00:00–01:00');
    assert.equal(formatBucketTime('2024-02-29','day'), '29 Feb 2024');
    assert.equal(formatBucketTime('2026-09-01','month'), 'September 2026');
    assert.equal(formatBucketTime('2026-02-29','day'), '2026-02-29');
    assert.equal(formatBucketTime('unknown','day'), 'unknown');
});
