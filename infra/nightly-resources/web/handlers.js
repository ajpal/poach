function toggle(elt, showText, hideText) {
  const content = elt.nextElementSibling;
  if (content.classList.contains("expanded")) {
    collapse(content, elt, showText);
  } else {
    expand(content, elt, hideText);
  }
}

function expand(elt, labelElt, text) {
  elt.classList.add("expanded");
  elt.classList.remove("collapsed");
  labelElt.innerText = text;
}

function collapse(elt, labelElt, text) {
  elt.classList.add("collapsed");
  elt.classList.remove("expanded");
  labelElt.innerText = text;
}
