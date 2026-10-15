import test from 'node:test';
import assert from 'node:assert/strict';
import {captureDimensions, exportFilename, pdfPage} from '../static/dashboard-export.js';

test('download filenames include the displayed scope and cannot create path components', () => {
    assert.equal(exportFilename('monthly','2026-09-01','Meter A'), 'telemetry-monthly-2026-09-01-Meter-A');
    const name = exportFilename('daily','2026-09-30','../../A:<B>"?*|\\C');
    assert.match(name,/^telemetry-daily-2026-09-30-/);
    assert.doesNotMatch(name,/[./\\:"?*|<>]/);
    assert.match(exportFilename('yearly','2026-01-01','พื้นที่ 1'),/พื้นที่-1$/u);
});

test('ordinary reports render at double resolution; long reports stay within memory bounds', () => {
    assert.equal(captureDimensions(1440,1500).pixelRatio,2);
    const dimensions = captureDimensions(1440,6000);
    assert.ok(dimensions.pixelRatio >= 1 && dimensions.pixelRatio < 2);
    assert.ok(dimensions.width * dimensions.height * dimensions.pixelRatio ** 2 <= 16_000_001);
    assert.throws(()=>captureDimensions(1440,20000),/too large/);
    assert.throws(()=>captureDimensions(1440,0),/no exportable/);
    assert.throws(()=>captureDimensions(1440,NaN),/no exportable/);
});

test('single-page PDF preserves the complete report aspect ratio in both orientations', () => {
    const portrait = pdfPage(1440,1500);
    assert.equal(portrait.orientation,'portrait');
    assert.equal(portrait.width,1080);
    assert.equal(portrait.width / portrait.height,1440 / 1500);
    const landscape = pdfPage(1440,800);
    assert.equal(landscape.orientation,'landscape');
    assert.equal(landscape.width / landscape.height,1440 / 800);
});
