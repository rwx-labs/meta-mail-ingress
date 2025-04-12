console.log("dddd");

const uploadForm = document.forms[0];

async function createUpload(files) {

}

uploadForm.addEventListener('submit', (event) => {
  const form = event.target;
  const fileInput = form.elements[0];
  const file = fileInput.files[0];

  console.log("file: %o", file);

  event.preventDefault();
});

console.log("form: %o", uploadForm);
