const images = [...document.querySelectorAll('img.animated')];
const toggle = document.querySelector('#motion-toggle');
const motionPreference = matchMedia('(prefers-reduced-motion: reduce)');
let paused = motionPreference.matches;
const frames = new WeakMap();

function syncFrame(img) {
  const previous = frames.get(img);
  if (!paused) {
    previous?.remove();
    frames.delete(img);
    img.hidden = false;
    return;
  }
  if (!img.complete || !img.naturalWidth) return;
  const canvas = previous ?? document.createElement('canvas');
  canvas.width = img.naturalWidth;
  canvas.height = img.naturalHeight;
  const context = canvas.getContext('2d');
  if (!context) return;
  context.drawImage(img, 0, 0);
  canvas.className = `${img.className} frozen-frame`;
  canvas.setAttribute('role', 'img');
  canvas.setAttribute('aria-label', img.alt);
  if (!previous) img.after(canvas);
  frames.set(img, canvas);
  img.hidden = true;
}

function syncMotion() {
  toggle.textContent = paused ? '播放动画 ▷' : '暂停动画 Ⅱ';
  toggle.setAttribute('aria-pressed', String(paused));
  images.forEach(syncFrame);
}

images.forEach(img => img.addEventListener('load', () => syncFrame(img)));
toggle.addEventListener('click', () => { paused = !paused; syncMotion(); });
motionPreference.addEventListener('change', event => { paused = event.matches; syncMotion(); });
syncMotion();

document.querySelectorAll('[data-pose]').forEach(button => {
  button.addEventListener('click', () => {
    document.querySelectorAll('[data-pose]').forEach(item => item.setAttribute('aria-pressed', String(item === button)));
    document.querySelectorAll('[data-agent]').forEach(img => {
      const label = img.closest('article').querySelector('h3').textContent;
      img.alt = `${label}：${button.textContent}`;
      img.loading = 'eager';
      img.src = `/assets/agents/${img.dataset.agent}-${button.dataset.pose}.webp`;
    });
    document.querySelector('#pose-announcement').textContent = `正在预览：${button.textContent}`;
  });
});

const platform = navigator.userAgentData?.platform ?? navigator.platform;
const current = /Mac/i.test(platform) ? 'mac' : /Win/i.test(platform) ? 'windows' : /Linux/i.test(platform) && !/Android/i.test(navigator.userAgent) ? 'linux' : null;
if (current && navigator.maxTouchPoints < 2) {
  const card = document.querySelector(`[data-platform="${current}"]`);
  card.classList.add('recommended');
  const hint = document.createElement('span');
  hint.className = 'platform-hint';
  hint.textContent = '你正在使用的平台';
  card.prepend(hint);
}
