// Botón flotante «Apoyar» del sitio de Tlaku. Sin dependencias ni red: solo muestra el botón que
// ya está en la página (oculto con `hidden`) cuando la persona baja, y recuerda si lo cerró.
(function () {
  'use strict';
  var KEY = 'tlaku-fab-cerrado';
  var fab = document.getElementById('fab');
  var section = document.getElementById('apoyar') || document.getElementById('support');
  if (!fab) return;

  // Sin un enlace real a Ko-fi (marcador sin llenar) no hay nada que ofrecer.
  var kofi = document.querySelector('a.kofi[href^="http"]');
  if (!kofi) return;

  function closed() {
    try { return window.localStorage.getItem(KEY) === '1'; } catch (e) { return false; }
  }
  var dismissed = closed();
  var scrolled = false;
  var inSection = false;

  function update() {
    fab.hidden = dismissed || !scrolled || inSection;
  }

  function onScroll() {
    scrolled = window.scrollY > 700;
    update();
  }

  var close = fab.querySelector('button');
  if (close) {
    close.addEventListener('click', function () {
      dismissed = true;
      try { window.localStorage.setItem(KEY, '1'); } catch (e) { /* sin almacenamiento: se oculta solo en esta visita */ }
      update();
    });
  }

  if (section && 'IntersectionObserver' in window) {
    new IntersectionObserver(function (entries) {
      inSection = entries[0].isIntersecting;
      update();
    }).observe(section);
  }

  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();
})();
