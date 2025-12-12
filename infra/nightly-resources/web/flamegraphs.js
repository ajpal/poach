function loadFlamegraphs() {
  fetch("flamegraphs.txt")
    .then((response) => response.text())
    .then((blob) => blob.split(/\r?\n/))
    .then((files) => {
      const listElt = document.getElementById("fileList");
      files.forEach((file) => {
        if (!file) return;
        const li = document.createElement("li");
        const p = document.createElement("p");
        p.textContent = file;
        const img = document.createElement("img");
        img.src = `flamegraphs/${file
          .split("/")
          .pop()
          .replace(/\.egg$/, ".svg")}`;
        img.alt = `flamegraph for ${file}`;

        // If the image fails to load
        img.onerror = () => {
          li.textContent = `No flamegraph for ${file}`;
        };

        li.appendChild(p);
        li.appendChild(img);
        listElt.appendChild(li);
      });
    });
}
